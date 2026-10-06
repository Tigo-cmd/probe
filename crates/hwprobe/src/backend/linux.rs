//! Linux backend: the stable sysfs ABI, procfs, and two read-only ioctls
//! (NVMe admin Get Log Page, and SG_IO ATA pass-through for SATA SMART).
//!
//! Both ioctls need root: the kernel blocks SG_IO below the capability check,
//! so CAP_SYS_RAWIO alone does not help. Unprivileged scans still return
//! inventory and battery data and record the missing health reads.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::is_placeholder;
use crate::model::*;
use crate::parse;

pub struct LinuxBackend {
    sys: PathBuf,
    proc_: PathBuf,
    dev: PathBuf,
    /// Issue device ioctls. Off when pointed at a fixture tree.
    device_io: bool,
}

impl LinuxBackend {
    pub fn system() -> Self {
        LinuxBackend {
            sys: "/sys".into(),
            proc_: "/proc".into(),
            dev: "/dev".into(),
            device_io: true,
        }
    }

    /// A backend that reads a captured sysfs/procfs tree and never touches devices.
    pub fn with_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        LinuxBackend {
            sys: root.join("sys"),
            proc_: root.join("proc"),
            dev: root.join("dev"),
            device_io: false,
        }
    }

    pub fn scan(&self) -> Scan {
        let start = Instant::now();
        let mut notes = Vec::new();
        let elevated = self.device_io && unsafe { libc::geteuid() } == 0;
        if self.device_io && !elevated {
            notes.push(note(
                "privilege",
                "not running as root: serials, UUID and drive health are unavailable",
            ));
        }

        let identity = self.identity();
        let mut scan = Scan {
            schema_version: SCAN_SCHEMA_VERSION,
            tool_version: crate::VERSION.to_string(),
            os: "linux".into(),
            started_at: crate::unix_now(),
            elevated,
            cpu: self.cpu(),
            memory: self.memory(),
            batteries: self.batteries(),
            storage: self.storage(&mut notes),
            displays: self.displays(&mut notes),
            network: self.network(),
            encumbrance: encumbrance(&identity),
            identity,
            ..Scan::default()
        };
        scan.probe_notes = notes;
        scan.duration_ms = start.elapsed().as_millis() as u64;
        scan
    }

    fn identity(&self) -> Identity {
        let dmi = |name: &str| self.dmi_string(name);
        Identity {
            vendor: dmi("sys_vendor"),
            model: dmi("product_name"),
            version: dmi("product_version"),
            serial: dmi("product_serial"),
            uuid: dmi("product_uuid"),
            board_vendor: dmi("board_vendor"),
            board_name: dmi("board_name"),
            board_serial: dmi("board_serial"),
            bios_vendor: dmi("bios_vendor"),
            bios_version: dmi("bios_version"),
            bios_date: dmi("bios_date"),
            asset_tag: dmi("chassis_asset_tag"),
            chassis_type: {
                let path = self.sys.join("class/dmi/id/chassis_type");
                Field::claimed(
                    read_trimmed(&path).and_then(|s| s.parse().ok()),
                    source(&path),
                )
            },
        }
    }

    fn dmi_string(&self, name: &str) -> Field<String> {
        let path = self.sys.join("class/dmi/id").join(name);
        match fs::read_to_string(&path) {
            Ok(s) if is_placeholder(&s) => Field::missing(
                Provenance::Claimed,
                source(&path),
                format!("OEM placeholder {:?}", s.trim()),
            ),
            Ok(s) => Field::claimed(Some(s.trim().to_string()), source(&path)),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Field::missing(Provenance::Claimed, source(&path), "requires root")
            }
            Err(_) => Field::claimed(None, source(&path)),
        }
    }

    fn cpu(&self) -> Cpu {
        let path = self.proc_.join("cpuinfo");
        let src = format!("cpuid via {}", source(&path));
        let Some(info) = read(&path) else {
            return Cpu {
                brand: Field::missing(Provenance::Measured, &src, "unreadable"),
                logical_cores: Field::missing(Provenance::Measured, &src, "unreadable"),
                physical_cores: Field::missing(Provenance::Measured, &src, "unreadable"),
            };
        };

        let mut brand = None;
        let mut logical = 0u32;
        let mut cores = std::collections::BTreeSet::new();
        let (mut package, mut core) = (None, None);
        for line in info.lines().chain(std::iter::once("")) {
            let (key, value) = line
                .split_once(':')
                .map(|(k, v)| (k.trim(), v.trim()))
                .unwrap_or((line.trim(), ""));
            match key {
                "processor" => logical += 1,
                "model name" if brand.is_none() => brand = Some(value.to_string()),
                "physical id" => package = Some(value.to_string()),
                "core id" => core = Some(value.to_string()),
                "" => {
                    if let (Some(p), Some(c)) = (package.take(), core.take()) {
                        cores.insert((p, c));
                    }
                }
                _ => {}
            }
        }
        Cpu {
            brand: Field::measured(brand, &src),
            logical_cores: Field::measured((logical > 0).then_some(logical), &src),
            physical_cores: Field::measured(
                (!cores.is_empty()).then_some(cores.len() as u32),
                &src,
            ),
        }
    }

    fn memory(&self) -> Memory {
        let path = self.proc_.join("meminfo");
        let total = read(&path).and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("MemTotal:"))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
                .map(|kb| kb * 1024)
        });
        Memory {
            total_bytes: Field::measured(total, source(&path))
                .with_note("usable RAM after firmware and GPU reservations, not installed RAM"),
        }
    }

    fn batteries(&self) -> Vec<Battery> {
        let mut out = Vec::new();
        for dir in list_dir(&self.sys.join("class/power_supply")) {
            if read_trimmed(&dir.join("type")).as_deref() != Some("Battery") {
                continue;
            }
            // Peripheral batteries (mice, headsets) report scope=Device.
            if read_trimmed(&dir.join("scope")).as_deref() == Some("Device") {
                continue;
            }
            out.push(self.battery(&dir));
        }
        out
    }

    fn battery(&self, dir: &Path) -> Battery {
        let s = |name: &str| {
            let p = dir.join(name);
            Field::claimed(read_trimmed(&p).filter(|v| !is_placeholder(v)), source(&p))
        };
        let num = |name: &str| read_trimmed(&dir.join(name)).and_then(|v| v.parse::<u64>().ok());

        // Prefer energy (µWh). Fall back to charge (µAh), converted to mWh
        // when the design voltage is known.
        let voltage_uv = num("voltage_min_design");
        let capacity = |energy: &str, charge: &str| -> Field<Capacity> {
            let src_e = dir.join(energy);
            if let Some(uwh) = num(energy) {
                return Field::measured(
                    Some(Capacity {
                        value: uwh / 1000,
                        unit: CapacityUnit::MilliwattHours,
                    }),
                    source(&src_e),
                );
            }
            let src_c = dir.join(charge);
            match (num(charge), voltage_uv) {
                (Some(uah), Some(uv)) => Field::measured(
                    Some(Capacity {
                        value: (uah as u128 * uv as u128 / 1_000_000_000) as u64,
                        unit: CapacityUnit::MilliwattHours,
                    }),
                    format!("{} × voltage_min_design", source(&src_c)),
                ),
                (Some(uah), None) => Field::measured(
                    Some(Capacity {
                        value: uah / 1000,
                        unit: CapacityUnit::MilliampHours,
                    }),
                    source(&src_c),
                ),
                _ => Field::measured(None, source(&src_e)),
            }
        };

        let mut design = capacity("energy_full_design", "charge_full_design");
        design.provenance = Provenance::Claimed; // a rewritable value in the pack's EEPROM
        let full = capacity("energy_full", "charge_full");

        let cycles_path = dir.join("cycle_count");
        let cycle_count = match num("cycle_count") {
            // The kernel and most firmware report 0 when unsupported.
            Some(0) => Field::missing(
                Provenance::Measured,
                source(&cycles_path),
                "reported as 0, treated as not reported",
            ),
            Some(n) => Field::measured(Some(n.min(u32::MAX as u64) as u32), source(&cycles_path)),
            None => Field::measured(None, source(&cycles_path)),
        };

        let date = match (
            num("manufacture_year"),
            num("manufacture_month"),
            num("manufacture_day"),
        ) {
            (Some(y), Some(m), Some(d)) => Some(format!("{y:04}-{m:02}-{d:02}")),
            _ => None,
        };

        Battery {
            name: file_name(dir),
            manufacturer: s("manufacturer"),
            model: s("model_name"),
            serial: s("serial_number"),
            technology: s("technology"),
            design_capacity: design,
            full_charge_capacity: full,
            cycle_count,
            manufacture_date: Field::claimed(date, source(&dir.join("manufacture_year"))),
        }
    }

    fn storage(&self, notes: &mut Vec<ProbeNote>) -> Vec<Drive> {
        let mut out = Vec::new();
        for dir in list_dir(&self.sys.join("block")) {
            let name = file_name(&dir);
            if ["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd"]
                .iter()
                .any(|p| name.starts_with(p))
            {
                continue;
            }
            let device_link = fs::canonicalize(dir.join("device")).ok();
            let device_path = device_link
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let transport = if device_path.contains("/usb") {
                Transport::Usb
            } else if name.starts_with("nvme") {
                Transport::Nvme
            } else if read_trimmed(&dir.join("device/vendor")).as_deref() == Some("ATA") {
                Transport::Sata
            } else {
                Transport::Other
            };

            let dev = dir.join("device");
            let string = |p: PathBuf| {
                Field::claimed(read_trimmed(&p).filter(|v| !is_placeholder(v)), source(&p))
            };
            let (model, serial, firmware) = match transport {
                Transport::Nvme => (
                    string(dev.join("model")),
                    string(dev.join("serial")),
                    string(dev.join("firmware_rev")),
                ),
                _ => (
                    string(dev.join("model")),
                    self.scsi_serial(&dev),
                    string(dev.join("rev")),
                ),
            };
            let size_path = dir.join("size");
            let sectors = read_trimmed(&size_path).and_then(|s| s.parse::<u64>().ok());
            if sectors == Some(0) {
                continue; // empty card reader slot or unattached virtual disk
            }

            let health = match transport {
                Transport::Usb => DriveHealth::Unavailable {
                    reason: "drive behind USB bridge: health data unverified".into(),
                },
                _ if !self.device_io => DriveHealth::Unavailable {
                    reason: "device I/O disabled".into(),
                },
                Transport::Nvme => {
                    let ctrl = device_link.as_deref().map(file_name).unwrap_or_default();
                    ioctl::nvme_health(&self.dev.join(&ctrl))
                }
                Transport::Sata => ioctl::ata_smart(&self.dev.join(&name)),
                Transport::Other => DriveHealth::Unavailable {
                    reason: "virtual or unsupported transport".into(),
                },
            };
            if let DriveHealth::Unavailable { reason } = &health {
                notes.push(note(&format!("storage/{name}"), reason));
            }

            out.push(Drive {
                name: name.clone(),
                transport,
                rotational: Field::measured(
                    read_trimmed(&dir.join("queue/rotational")).map(|v| v == "1"),
                    source(&dir.join("queue/rotational")),
                ),
                removable: read_trimmed(&dir.join("removable")).as_deref() == Some("1"),
                model,
                serial,
                firmware,
                capacity_bytes: Field::claimed(sectors.map(|s| s * 512), source(&size_path))
                    .with_note("reported by the drive controller; not verified by write-then-read"),
                health,
            });
        }
        out
    }

    /// SCSI/SATA unit serial from VPD page 0x80, if the kernel exposes it.
    fn scsi_serial(&self, dev: &Path) -> Field<String> {
        let path = dev.join("vpd_pg80");
        let value = fs::read(&path).ok().filter(|b| b.len() > 4).and_then(|b| {
            let len = (b[3] as usize).min(b.len() - 4);
            let s = String::from_utf8_lossy(&b[4..4 + len]).trim().to_string();
            (!is_placeholder(&s)).then_some(s)
        });
        Field::claimed(value, source(&path))
    }

    fn displays(&self, notes: &mut Vec<ProbeNote>) -> Vec<Display> {
        let mut out = Vec::new();
        for dir in list_dir(&self.sys.join("class/drm")) {
            let name = file_name(&dir);
            // Connectors look like card0-eDP-1; skip the card nodes themselves.
            let Some((_, connector)) = name.split_once('-') else {
                continue;
            };
            let Ok(bytes) = fs::read(dir.join("edid")) else {
                continue;
            };
            if bytes.is_empty() {
                continue; // nothing connected
            }
            match parse::edid::parse_edid(connector, &bytes) {
                Ok(d) => out.push(d),
                Err(e) => notes.push(note(
                    &format!("display/{connector}"),
                    &format!("EDID rejected: {e}"),
                )),
            }
        }
        out
    }

    fn network(&self) -> Vec<NetworkInterface> {
        list_dir(&self.sys.join("class/net"))
            .into_iter()
            // Only interfaces backed by a physical device; skips lo, bridges, VPNs.
            .filter(|dir| dir.join("device").exists())
            .filter_map(|dir| {
                let mac = read_trimmed(&dir.join("address"))?;
                (mac != "00:00:00:00:00:00").then(|| NetworkInterface {
                    name: file_name(&dir),
                    mac,
                })
            })
            .collect()
    }
}

/// Encumbrance signals readable on Linux. The strong ones (Autopilot, MDM
/// enrolment, Activation Lock, Absolute persistence) live in Windows, macOS
/// and firmware and are not visible from here.
fn encumbrance(identity: &Identity) -> Vec<EncumbranceSignal> {
    let tag = identity.asset_tag.get();
    vec![EncumbranceSignal {
        id: "smbios_asset_tag".into(),
        present: tag.is_some(),
        provenance: Provenance::Claimed,
        source: identity.asset_tag.source.clone(),
        detail: match tag {
            Some(t) => format!("SMBIOS asset tag is set ({t:?}): ex-corporate marker"),
            None => "no SMBIOS asset tag".into(),
        },
    }]
}

mod ioctl {
    //! The two device reads the Linux backend issues. Both are read-only.

    use std::fs::File;
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    use crate::model::DriveHealth;
    use crate::parse;

    /// `struct nvme_passthru_cmd` from <linux/nvme_ioctl.h>.
    #[repr(C)]
    #[derive(Default)]
    struct NvmePassthruCmd {
        opcode: u8,
        flags: u8,
        rsvd1: u16,
        nsid: u32,
        cdw2: u32,
        cdw3: u32,
        metadata: u64,
        addr: u64,
        metadata_len: u32,
        data_len: u32,
        cdw10: u32,
        cdw11: u32,
        cdw12: u32,
        cdw13: u32,
        cdw14: u32,
        cdw15: u32,
        timeout_ms: u32,
        result: u32,
    }

    // _IOWR('N', 0x41, struct nvme_admin_cmd)
    const NVME_IOCTL_ADMIN_CMD: libc::c_ulong = 0xC048_4E41;
    const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;
    const NVME_LOG_HEALTH: u32 = 0x02;

    pub fn nvme_health(ctrl: &Path) -> DriveHealth {
        let file = match File::open(ctrl) {
            Ok(f) => f,
            Err(e) => return unavailable(ctrl, e),
        };
        let mut buf = [0u8; parse::nvme::HEALTH_LOG_LEN];
        let numd = (buf.len() / 4 - 1) as u32;
        let mut cmd = NvmePassthruCmd {
            opcode: NVME_ADMIN_GET_LOG_PAGE,
            nsid: 0xFFFF_FFFF,
            addr: buf.as_mut_ptr() as u64,
            data_len: buf.len() as u32,
            cdw10: (numd << 16) | NVME_LOG_HEALTH,
            ..Default::default()
        };
        // SAFETY: cmd matches the kernel ABI and addr points to a live buffer of data_len bytes.
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), NVME_IOCTL_ADMIN_CMD as _, &mut cmd) };
        if rc != 0 {
            return unavailable(ctrl, std::io::Error::last_os_error());
        }
        match parse::nvme::parse_health_log(&buf) {
            Ok(h) => DriveHealth::Nvme(h),
            Err(e) => DriveHealth::Unavailable {
                reason: format!("NVMe health log rejected: {e}"),
            },
        }
    }

    /// `struct sg_io_hdr` from <scsi/sg.h>.
    #[repr(C)]
    struct SgIoHdr {
        interface_id: libc::c_int,
        dxfer_direction: libc::c_int,
        cmd_len: libc::c_uchar,
        mx_sb_len: libc::c_uchar,
        iovec_count: libc::c_ushort,
        dxfer_len: libc::c_uint,
        dxferp: *mut libc::c_void,
        cmdp: *mut libc::c_uchar,
        sbp: *mut libc::c_uchar,
        timeout: libc::c_uint,
        flags: libc::c_uint,
        pack_id: libc::c_int,
        usr_ptr: *mut libc::c_void,
        status: libc::c_uchar,
        masked_status: libc::c_uchar,
        msg_status: libc::c_uchar,
        sb_len_wr: libc::c_uchar,
        host_status: libc::c_ushort,
        driver_status: libc::c_ushort,
        resid: libc::c_int,
        duration: libc::c_uint,
        info: libc::c_uint,
    }

    const SG_IO: libc::c_ulong = 0x2285;
    const SG_DXFER_FROM_DEV: libc::c_int = -3;

    pub fn ata_smart(dev: &Path) -> DriveHealth {
        let file = match File::open(dev) {
            Ok(f) => f,
            Err(e) => return unavailable(dev, e),
        };
        let mut data = [0u8; parse::ata::SMART_DATA_LEN];
        let mut sense = [0u8; 32];
        // ATA PASS-THROUGH (16): PIO data-in, SMART READ DATA (B0h, feature D0h).
        let mut cdb: [u8; 16] = [
            0x85,
            4 << 1,
            0x0E,
            0,
            0xD0,
            0,
            1,
            0,
            0,
            0,
            0x4F,
            0,
            0xC2,
            0,
            0xB0,
            0,
        ];
        let mut hdr = SgIoHdr {
            interface_id: b'S' as libc::c_int,
            dxfer_direction: SG_DXFER_FROM_DEV,
            cmd_len: cdb.len() as u8,
            mx_sb_len: sense.len() as u8,
            iovec_count: 0,
            dxfer_len: data.len() as u32,
            dxferp: data.as_mut_ptr().cast(),
            cmdp: cdb.as_mut_ptr(),
            sbp: sense.as_mut_ptr(),
            timeout: 10_000,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        };
        // SAFETY: hdr matches the kernel ABI; all pointers reference live buffers of the stated lengths.
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), SG_IO as _, &mut hdr) };
        if rc != 0 {
            return unavailable(dev, std::io::Error::last_os_error());
        }
        if hdr.status != 0 || hdr.host_status != 0 {
            return DriveHealth::Unavailable {
                reason: format!("{}: SMART READ DATA failed", dev.display()),
            };
        }
        match parse::ata::parse_smart_data(&data) {
            Ok(h) => DriveHealth::Ata(h),
            Err(e) => DriveHealth::Unavailable {
                reason: format!("SMART data rejected: {e}"),
            },
        }
    }

    fn unavailable(dev: &Path, e: std::io::Error) -> DriveHealth {
        let reason = if e.kind() == std::io::ErrorKind::PermissionDenied
            || e.raw_os_error() == Some(libc::EPERM)
        {
            format!("{}: requires root", dev.display())
        } else {
            format!("{}: {e}", dev.display())
        };
        DriveHealth::Unavailable { reason }
    }
}

fn note(component: &str, message: &str) -> ProbeNote {
    ProbeNote {
        component: component.into(),
        message: message.into(),
    }
}

fn source(path: &Path) -> String {
    format!("sysfs:{}", path.display())
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn read_trimmed(path: &Path) -> Option<String> {
    read(path)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn list_dir(path: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(path)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    entries
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("hwprobe-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Tree(root)
        }
        fn put(&self, rel: &str, contents: &str) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, contents).unwrap();
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scans_fixture_tree() {
        let t = Tree::new("fixture");
        t.put("sys/class/dmi/id/sys_vendor", "LENOVO\n");
        t.put("sys/class/dmi/id/product_name", "20AQ0069UK\n");
        t.put(
            "sys/class/dmi/id/product_serial",
            "To be filled by O.E.M.\n",
        );
        t.put("sys/class/dmi/id/chassis_asset_tag", "ACME-IT-00412\n");
        t.put("sys/class/dmi/id/chassis_type", "10\n");
        t.put(
            "proc/cpuinfo",
            "processor\t: 0\nmodel name\t: Intel(R) Core(TM) i5-4300U CPU @ 1.90GHz\nphysical id\t: 0\ncore id\t: 0\n\n\
             processor\t: 1\nmodel name\t: Intel(R) Core(TM) i5-4300U CPU @ 1.90GHz\nphysical id\t: 0\ncore id\t: 0\n\n\
             processor\t: 2\nphysical id\t: 0\ncore id\t: 1\n\n\
             processor\t: 3\nphysical id\t: 0\ncore id\t: 1\n",
        );
        t.put(
            "proc/meminfo",
            "MemTotal:        3946100 kB\nMemFree: 1 kB\n",
        );
        t.put("sys/class/power_supply/BAT0/type", "Battery\n");
        t.put(
            "sys/class/power_supply/BAT0/energy_full_design",
            "23200000\n",
        );
        t.put("sys/class/power_supply/BAT0/energy_full", "17400000\n");
        t.put("sys/class/power_supply/BAT0/cycle_count", "0\n");
        t.put("sys/class/power_supply/BAT0/manufacturer", "SANYO\n");
        t.put("sys/class/power_supply/hidpp_battery_0/type", "Battery\n");
        t.put("sys/class/power_supply/hidpp_battery_0/scope", "Device\n");
        t.put("sys/class/power_supply/AC/type", "Mains\n");

        let scan = LinuxBackend::with_root(&t.0).scan();

        assert_eq!(
            scan.identity.vendor.get().map(String::as_str),
            Some("LENOVO")
        );
        assert_eq!(scan.identity.serial.value, None);
        assert!(scan
            .identity
            .serial
            .note
            .as_deref()
            .unwrap()
            .contains("placeholder"));
        assert_eq!(scan.identity.serial.provenance, Provenance::Claimed);
        assert_eq!(scan.identity.chassis_type.value, Some(10));

        assert_eq!(scan.cpu.logical_cores.value, Some(4));
        assert_eq!(scan.cpu.physical_cores.value, Some(2));
        assert_eq!(scan.cpu.brand.provenance, Provenance::Measured);
        assert_eq!(scan.memory.total_bytes.value, Some(3_946_100 * 1024));

        assert_eq!(
            scan.batteries.len(),
            1,
            "peripheral battery and AC adapter are skipped"
        );
        let b = &scan.batteries[0];
        assert_eq!(b.design_capacity.value.unwrap().value, 23_200);
        assert_eq!(b.design_capacity.provenance, Provenance::Claimed);
        assert_eq!(b.full_charge_capacity.value.unwrap().value, 17_400);
        assert_eq!(b.cycle_count.value, None, "zero cycles means not reported");

        let tag = &scan.encumbrance[0];
        assert!(tag.present);
        assert!(tag.detail.contains("ACME-IT-00412"));
    }

    #[test]
    fn converts_charge_to_energy_with_design_voltage() {
        let t = Tree::new("charge");
        t.put("sys/class/power_supply/BAT1/type", "Battery\n");
        t.put(
            "sys/class/power_supply/BAT1/charge_full_design",
            "5000000\n",
        );
        t.put("sys/class/power_supply/BAT1/charge_full", "4000000\n");
        t.put(
            "sys/class/power_supply/BAT1/voltage_min_design",
            "11100000\n",
        );
        let scan = LinuxBackend::with_root(&t.0).scan();
        let b = &scan.batteries[0];
        assert_eq!(
            b.design_capacity.value,
            Some(Capacity {
                value: 55_500,
                unit: CapacityUnit::MilliwattHours
            })
        );
        assert_eq!(b.full_charge_capacity.value.unwrap().value, 44_400);
    }
}
