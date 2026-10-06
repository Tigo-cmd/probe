//! macOS backend: the I/O Registry (`ioreg -a`), `system_profiler -xml`,
//! `profiles status`, `firmwarepasswd` and `sysctl`. All ship with macOS, so
//! the single-binary, offline model holds; no kext, no private framework.
//!
//! The tools print property lists or plain text. Everything that interprets
//! that output is in this file but outside the `sys` module, so it compiles
//! and is tested on every platform. Only `sys` runs commands.
//!
//! Limits, reported rather than hidden: macOS exposes only a pass/fail SMART
//! summary for internal drives, the logic-board serial is not published, and
//! Macs carry no SMBIOS asset tag.

use super::is_placeholder;
use super::signals::ActivationLock;
use crate::model::*;
use crate::parse::plist::Value;

const PLATFORM_SRC: &str = "ioreg:IOPlatformExpertDevice";
const HARDWARE_SRC: &str = "system_profiler:SPHardwareDataType";
const BATTERY_SRC: &str = "ioreg:AppleSmartBattery";

#[cfg(target_os = "macos")]
pub use sys::MacBackend;

/// The `_items` of one data type in `system_profiler -xml` output.
fn profiler_items<'a>(root: &'a Value, data_type: &str) -> &'a [Value] {
    root.as_array()
        .iter()
        .find(|d| d.get("_dataType").and_then(Value::as_str) == Some(data_type))
        .and_then(|d| d.get("_items"))
        .map(Value::as_array)
        .unwrap_or(&[])
}

/// A printable identity string, or `None` for placeholders and binary junk.
fn clean(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::text)
        .filter(|s| s.chars().all(|c| !c.is_control()))
        .filter(|s| !is_placeholder(s))
}

fn claimed(v: Option<String>, src: &str, key: &str) -> Field<String> {
    Field::claimed(v, format!("{src}/{key}"))
}

fn not_exposed(src: &str, why: &str) -> Field<String> {
    Field::missing(Provenance::Claimed, src, why)
}

// ---------------------------------------------------------------- identity

/// `platform` is the IOPlatformExpertDevice node; `hardware` the first
/// SPHardwareDataType item. Either may be absent.
pub fn identity(platform: Option<&Value>, hardware: Option<&Value>) -> Identity {
    let p = |k: &str| clean(platform.and_then(|v| v.get(k)));
    let h = |k: &str| clean(hardware.and_then(|v| v.get(k)));
    let either = |pk: &str, hk: &str| match p(pk) {
        Some(v) => claimed(Some(v), PLATFORM_SRC, pk),
        None => claimed(h(hk), HARDWARE_SRC, hk),
    };
    let mut uuid = either("IOPlatformUUID", "platform_UUID");
    uuid.value = uuid.value.map(|u| u.to_ascii_lowercase());

    Identity {
        vendor: claimed(p("manufacturer"), PLATFORM_SRC, "manufacturer"),
        model: either("model", "machine_model"),
        version: not_exposed(PLATFORM_SRC, "not published by macOS"),
        serial: either("IOPlatformSerialNumber", "serial_number"),
        uuid,
        board_vendor: not_exposed(PLATFORM_SRC, "not published by macOS"),
        board_name: claimed(p("board-id"), PLATFORM_SRC, "board-id"),
        board_serial: not_exposed(
            PLATFORM_SRC,
            "the logic-board serial is not published by macOS",
        ),
        bios_vendor: not_exposed(PLATFORM_SRC, "not published by macOS"),
        bios_version: claimed(h("boot_rom_version"), HARDWARE_SRC, "boot_rom_version"),
        bios_date: not_exposed(PLATFORM_SRC, "not published by macOS"),
        asset_tag: not_exposed(PLATFORM_SRC, "Macs carry no SMBIOS asset tag"),
        chassis_type: Field::missing(Provenance::Claimed, PLATFORM_SRC, "not published by macOS"),
    }
}

// ----------------------------------------------------------------- battery

/// One AppleSmartBattery node. Capacities are in mAh.
pub fn battery(node: &Value, index: usize) -> Battery {
    let data = node.get("BatteryData");
    let int = |k: &str| {
        node.get(k)
            .or_else(|| data.and_then(|d| d.get(k)))
            .and_then(Value::as_int)
            .filter(|&v| v > 0 && v <= u32::MAX as i128)
            .map(|v| v as u64)
    };
    let mah = |v: Option<u64>| {
        v.map(|value| Capacity {
            value,
            unit: CapacityUnit::MilliampHours,
        })
    };
    let src = |k: &str| format!("{BATTERY_SRC}/{k}");

    let design = int("DesignCapacity");
    let full = match int("AppleRawMaxCapacity") {
        Some(raw) => Field::measured(mah(Some(raw)), src("AppleRawMaxCapacity")),
        // On Apple silicon MaxCapacity is a percentage; only Intel Macs
        // report it in mAh. A value that small next to a real design figure
        // is the percentage and must not be graded as a capacity.
        None => match int("MaxCapacity") {
            Some(m) if m <= 100 && design.is_some_and(|d| d > 100) => Field::missing(
                Provenance::Measured,
                src("MaxCapacity"),
                "reported as a percentage, not a capacity",
            ),
            m => Field::measured(mah(m), src("MaxCapacity")),
        },
    };
    let cycle_count = match int("CycleCount") {
        Some(n) => Field::measured(Some(n as u32), src("CycleCount")),
        None => Field::missing(
            Provenance::Measured,
            src("CycleCount"),
            "not reported, or reported as 0",
        ),
    };
    let text = |k: &str| clean(node.get(k).or_else(|| data.and_then(|d| d.get(k))));
    let serial = text("Serial").or_else(|| text("BatterySerialNumber"));

    Battery {
        name: format!("BAT{index}"),
        manufacturer: Field::claimed(text("Manufacturer"), src("Manufacturer")),
        model: Field::claimed(text("DeviceName"), src("DeviceName")),
        serial: Field::claimed(serial, src("Serial")),
        technology: Field::claimed(None, src("Technology")),
        design_capacity: Field::claimed(mah(design), src("DesignCapacity")),
        full_charge_capacity: full,
        cycle_count,
        manufacture_date: Field::claimed(
            int("ManufactureDate").and_then(smart_battery_date),
            src("ManufactureDate"),
        ),
    }
}

/// Smart Battery Data date: day + month × 32 + (year − 1980) × 512.
fn smart_battery_date(v: u64) -> Option<String> {
    if v > u16::MAX as u64 {
        return None;
    }
    let (day, month, year) = (v & 0x1F, (v >> 5) & 0x0F, 1980 + (v >> 9));
    ((1..=31).contains(&day) && (1..=12).contains(&month) && year <= 2100)
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

// ----------------------------------------------------------------- storage

/// Internal NVMe and SATA drives from `system_profiler -xml`.
pub fn drives(profiler: &Value) -> Vec<Drive> {
    let mut out = Vec::new();
    for (data_type, transport) in [
        ("SPNVMeDataType", Transport::Nvme),
        ("SPSerialATADataType", Transport::Sata),
    ] {
        let src = format!("system_profiler:{data_type}");
        for item in profiler_items(profiler, data_type) {
            for d in item.dicts() {
                if d.get("device_model").is_none() || d.get("size_in_bytes").is_none() {
                    continue; // a controller, not a drive
                }
                let field = |k: &str| Field::claimed(clean(d.get(k)), format!("{src}/{k}"));
                let yes = |k: &str| d.get(k).and_then(Value::as_str) == Some("yes");
                let name = clean(d.get("bsd_name"))
                    .or_else(|| clean(d.get("_name")))
                    .unwrap_or_else(|| format!("disk{}", out.len()));
                let rotational = match transport {
                    Transport::Nvme => Some(false),
                    _ => d
                        .get("spsata_medium_type")
                        .and_then(Value::as_str)
                        .map(|m| m.eq_ignore_ascii_case("rotational")),
                };
                let smart = clean(d.get("smart_status"))
                    .map(|s| format!("SMART summary {s:?}"))
                    .unwrap_or_else(|| "no SMART summary".into());
                out.push(Drive {
                    name,
                    transport: if yes("detachable_drive") {
                        Transport::Usb
                    } else {
                        transport
                    },
                    rotational: Field::measured(rotational, format!("{src}/spsata_medium_type")),
                    removable: yes("removable_media"),
                    model: field("device_model"),
                    serial: field("device_serial"),
                    firmware: field("device_revision"),
                    capacity_bytes: Field::claimed(
                        d.get("size_in_bytes")
                            .and_then(Value::as_int)
                            .filter(|&b| b > 0 && b <= u64::MAX as i128)
                            .map(|b| b as u64),
                        format!("{src}/size_in_bytes"),
                    )
                    .with_note("reported by the drive controller; not verified by write-then-read"),
                    health: DriveHealth::Unavailable {
                        reason: format!(
                            "macOS exposes only a pass/fail SMART summary ({smart}); the health log was not read"
                        ),
                    },
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------- network, display

/// Burned-in MACs from `ioreg -a -r -c IOEthernetController`. Wi-Fi
/// controllers are a subclass, so they are included.
pub fn network(controllers: &Value) -> Vec<NetworkInterface> {
    let mut out: Vec<NetworkInterface> = Vec::new();
    for c in controllers.dicts() {
        let Some(mac) = c.get("IOMACAddress").and_then(Value::as_data) else {
            continue;
        };
        if mac.len() != 6 || mac.iter().all(|&b| b == 0) {
            continue;
        }
        let iface = c
            .get("IORegistryEntryChildren")
            .map(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .find(|i| i.get("BSD Name").is_some());
        // Skip adapters macOS says are not built in (USB and Thunderbolt dongles).
        if iface
            .and_then(|i| i.get("IOBuiltin"))
            .and_then(Value::as_bool)
            == Some(false)
        {
            continue;
        }
        let mac = mac
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":");
        if out.iter().any(|n| n.mac == mac) {
            continue;
        }
        out.push(NetworkInterface {
            name: iface
                .and_then(|i| clean(i.get("BSD Name")))
                .unwrap_or_else(|| "ethernet".into()),
            mac,
        });
    }
    out
}

/// Every EDID blob found in an I/O Registry tree.
pub fn displays(tree: &Value, notes: &mut Vec<ProbeNote>) -> Vec<Display> {
    let mut out = Vec::new();
    for d in tree.dicts() {
        let Value::Dict(entries) = d else { continue };
        for (key, v) in entries {
            if key != "IODisplayEDID" && key != "EDID" {
                continue;
            }
            let Some(bytes) = v.as_data() else { continue };
            let connector = format!("display{}", out.len());
            match crate::parse::edid::parse_edid(&connector, bytes) {
                Ok(display) => out.push(display),
                Err(e) => notes.push(ProbeNote {
                    component: format!("display/{connector}"),
                    message: format!("EDID rejected: {e}"),
                }),
            }
        }
    }
    out
}

// ------------------------------------------------------------- encumbrance

/// Activation Lock from the hardware overview. `None` means the Mac supports
/// it but its state was not reported: unknown, never "off".
pub fn activation_lock(hardware: Option<&Value>, ibridge: &[Value]) -> Option<ActivationLock> {
    let status = hardware
        .and_then(|h| h.get("activation_lock_status"))
        .and_then(Value::text)
        .map(|s| s.to_ascii_lowercase());
    match status {
        Some(s) if s.contains("disabled") => return Some(ActivationLock::Disabled),
        Some(s) if s.contains("enabled") => return Some(ActivationLock::Enabled),
        _ => {}
    }
    let apple_silicon = hardware
        .and_then(|h| clean(h.get("chip_type")))
        .is_some_and(|c| c.starts_with("Apple"));
    let t2 = ibridge
        .iter()
        .flat_map(Value::dicts)
        .any(|d| clean(d.get("ibridge_model_name")).is_some_and(|m| m.contains("T2")));
    if apple_silicon || t2 {
        None
    } else if hardware.is_some() {
        Some(ActivationLock::Unsupported)
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnrolmentStatus {
    pub automated: Option<bool>,
    /// Whether enrolled, and macOS's own wording, e.g. "Yes (User Approved)".
    pub mdm: Option<(bool, String)>,
}

/// Output of `profiles status -type enrollment`.
pub fn parse_profiles_status(text: &str) -> EnrolmentStatus {
    let mut s = EnrolmentStatus::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        let yes = v.to_ascii_lowercase().starts_with("yes");
        match k.trim().to_ascii_lowercase().as_str() {
            "enrolled via dep" => s.automated = Some(yes),
            "mdm enrollment" => s.mdm = Some((yes, v.to_string())),
            _ => {}
        }
    }
    s
}

/// Output of `firmwarepasswd -check`: "Password Enabled: Yes".
pub fn parse_firmwarepasswd(text: &str) -> Option<bool> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("password enabled")
            .then(|| v.trim().eq_ignore_ascii_case("yes"))
    })
}

#[cfg(target_os = "macos")]
mod sys {
    //! The only part of the macOS backend that touches the system.

    use std::process::Command;
    use std::time::Instant;

    use super::super::signals;
    use super::*;
    use crate::parse::plist;

    const PROFILES_SRC: &str = "profiles status -type enrollment";
    const FIRMWAREPASSWD_SRC: &str = "firmwarepasswd -check";

    pub struct MacBackend {
        _private: (),
    }

    impl MacBackend {
        pub fn system() -> Self {
            MacBackend { _private: () }
        }

        pub fn scan(&self) -> Scan {
            let start = Instant::now();
            let mut notes = Vec::new();
            // SAFETY: geteuid has no preconditions.
            let elevated = unsafe { libc::geteuid() } == 0;
            if !elevated {
                notes.push(note(
                    "privilege",
                    "not running as root: the firmware password state is unavailable",
                ));
            }

            let platform = ioreg(&["-d", "1", "-c", "IOPlatformExpertDevice"], &mut notes);
            let profiler = profiler(&mut notes);
            let hw_items = profiler
                .as_ref()
                .map(|p| profiler_items(p, "SPHardwareDataType"))
                .unwrap_or(&[]);
            let hardware = hw_items.first();
            let platform_node = platform.as_ref().and_then(|p| p.as_array().first());

            let identity = identity(platform_node, hardware);
            let batteries = ioreg(&["-c", "AppleSmartBattery"], &mut notes)
                .map(|t| {
                    t.as_array()
                        .iter()
                        .enumerate()
                        .map(|(i, n)| battery(n, i))
                        .collect()
                })
                .unwrap_or_default();
            let storage = profiler.as_ref().map(drives).unwrap_or_default();
            for d in &storage {
                if let DriveHealth::Unavailable { reason } = &d.health {
                    notes.push(note(&format!("storage/{}", d.name), reason));
                }
            }
            let network = ioreg(&["-c", "IOEthernetController"], &mut notes)
                .map(|t| network(&t))
                .unwrap_or_default();
            let mut displays = Vec::new();
            for class in ["IODisplayConnect", "AppleCLCD2"] {
                if let Some(t) = ioreg(&["-c", class], &mut notes) {
                    displays.extend(super::displays(&t, &mut notes));
                }
            }
            let ibridge = profiler
                .as_ref()
                .map(|p| profiler_items(p, "SPiBridgeDataType"))
                .unwrap_or(&[]);
            let encumbrance = encumbrance(hardware, ibridge, elevated, &mut notes);

            let mut scan = Scan {
                schema_version: SCAN_SCHEMA_VERSION,
                tool_version: crate::VERSION.to_string(),
                os: "macos".into(),
                started_at: crate::unix_now(),
                elevated,
                cpu: cpu(),
                memory: memory(),
                identity,
                batteries,
                storage,
                displays,
                network,
                encumbrance,
                ..Scan::default()
            };
            scan.probe_notes = notes;
            scan.duration_ms = start.elapsed().as_millis() as u64;
            scan
        }
    }

    fn encumbrance(
        hardware: Option<&Value>,
        ibridge: &[Value],
        elevated: bool,
        notes: &mut Vec<ProbeNote>,
    ) -> Vec<EncumbranceSignal> {
        let mut out = Vec::new();
        match activation_lock(hardware, ibridge) {
            Some(state) => out.push(signals::activation_lock(
                state,
                &format!("{HARDWARE_SRC}/activation_lock_status"),
            )),
            None => notes.push(note(
                "encumbrance/activation_lock",
                "Activation Lock state not reported by system_profiler",
            )),
        }

        match run("/usr/bin/profiles", &["status", "-type", "enrollment"]) {
            Ok(text) => {
                let s = parse_profiles_status(&text);
                match s.automated {
                    Some(a) => out.push(signals::automated_enrolment(a, PROFILES_SRC)),
                    None => notes.push(note(
                        "encumbrance/automated_device_enrolment",
                        "not reported by profiles",
                    )),
                }
                match s.mdm {
                    Some((enrolled, status)) => {
                        out.push(signals::mdm_status(enrolled, &status, PROFILES_SRC))
                    }
                    None => notes.push(note(
                        "encumbrance/mdm_enrolment",
                        "not reported by profiles",
                    )),
                }
            }
            Err(e) => notes.push(note("encumbrance/mdm_enrolment", &e)),
        }

        // Firmware passwords exist only on Intel Macs; Apple silicon has none.
        let apple_silicon = hardware
            .and_then(|h| h.get("chip_type"))
            .and_then(Value::text)
            .is_some_and(|c| c.starts_with("Apple"));
        if !apple_silicon {
            if !elevated {
                notes.push(note(
                    "encumbrance/firmware_password",
                    "firmwarepasswd requires root: not checked",
                ));
            } else {
                match run("/usr/sbin/firmwarepasswd", &["-check"]).map(|t| parse_firmwarepasswd(&t))
                {
                    Ok(Some(set)) => out.push(signals::firmware_password(set, FIRMWAREPASSWD_SRC)),
                    Ok(None) => notes.push(note(
                        "encumbrance/firmware_password",
                        "unrecognised firmwarepasswd output",
                    )),
                    Err(e) => notes.push(note("encumbrance/firmware_password", &e)),
                }
            }
        }
        out
    }

    fn note(component: &str, message: &str) -> ProbeNote {
        ProbeNote {
            component: component.into(),
            message: message.into(),
        }
    }

    fn run(program: &str, args: &[&str]) -> Result<String, String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "{program} {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// `ioreg -a -r` with the given selection. No match prints nothing, which
    /// is an empty result, not an error.
    fn ioreg(args: &[&str], notes: &mut Vec<ProbeNote>) -> Option<Value> {
        let mut full = vec!["-a", "-r"];
        full.extend_from_slice(args);
        let parsed = run("/usr/sbin/ioreg", &full).and_then(|text| {
            if text.trim().is_empty() {
                Ok(Value::Array(Vec::new()))
            } else {
                plist::parse(&text).map_err(|e| e.to_string())
            }
        });
        match parsed {
            Ok(v) => Some(v),
            Err(e) => {
                notes.push(note("ioreg", &format!("{}: {e}", args.join(" "))));
                None
            }
        }
    }

    fn profiler(notes: &mut Vec<ProbeNote>) -> Option<Value> {
        let parsed = run(
            "/usr/sbin/system_profiler",
            &[
                "-xml",
                "SPHardwareDataType",
                "SPiBridgeDataType",
                "SPNVMeDataType",
                "SPSerialATADataType",
            ],
        )
        .and_then(|text| plist::parse(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(v) => Some(v),
            Err(e) => {
                notes.push(note("system_profiler", &e));
                None
            }
        }
    }

    fn sysctl_bytes(name: &str) -> Option<Vec<u8>> {
        let cname = std::ffi::CString::new(name).ok()?;
        let mut len = 0usize;
        // SAFETY: a null buffer asks for the required length.
        let rc = unsafe {
            libc::sysctlbyname(
                cname.as_ptr(),
                std::ptr::null_mut(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        // SAFETY: buf is `len` bytes long.
        let rc = unsafe {
            libc::sysctlbyname(
                cname.as_ptr(),
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (rc == 0).then(|| {
            buf.truncate(len);
            buf
        })
    }

    fn sysctl_u64(name: &str) -> Option<u64> {
        let b = sysctl_bytes(name)?;
        match b.len() {
            4 => Some(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as u64),
            8 => Some(u64::from_ne_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ])),
            _ => None,
        }
    }

    fn cpu() -> Cpu {
        let brand = sysctl_bytes("machdep.cpu.brand_string").and_then(|b| {
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            let s = String::from_utf8_lossy(&b[..end]).trim().to_string();
            (!s.is_empty()).then_some(s)
        });
        let count = |name: &str| sysctl_u64(name).filter(|&n| n > 0).map(|n| n as u32);
        Cpu {
            brand: Field::measured(brand, "sysctl:machdep.cpu.brand_string"),
            logical_cores: Field::measured(count("hw.logicalcpu"), "sysctl:hw.logicalcpu"),
            physical_cores: Field::measured(count("hw.physicalcpu"), "sysctl:hw.physicalcpu"),
        }
    }

    fn memory() -> Memory {
        Memory {
            total_bytes: Field::measured(sysctl_u64("hw.memsize"), "sysctl:hw.memsize")
                .with_note("installed RAM as reported by the kernel"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::plist::parse;

    fn plist(body: &str) -> Value {
        parse(&format!("<plist version=\"1.0\">{body}</plist>")).unwrap()
    }

    #[test]
    fn identity_prefers_the_registry_and_lowercases_the_uuid() {
        let platform = plist(
            "<dict>\
             <key>IOPlatformSerialNumber</key><string>C02XK0AAJG5H</string>\
             <key>IOPlatformUUID</key><string>4C4C4544-0042-3510-8051-B4C04F4E4E32</string>\
             <key>manufacturer</key><data>QXBwbGUgSW5jLgA=</data>\
             <key>model</key><data>TWFjQm9va1BybzE1LDIA</data>\
             <key>board-id</key><data>AQIDBA==</data>\
             </dict>",
        );
        let hardware = plist(
            "<dict><key>machine_model</key><string>MacBookPro99,9</string>\
             <key>boot_rom_version</key><string>1715.81.2.0.0</string></dict>",
        );
        let id = identity(Some(&platform), Some(&hardware));
        assert_eq!(id.serial.get().map(String::as_str), Some("C02XK0AAJG5H"));
        assert_eq!(id.model.get().map(String::as_str), Some("MacBookPro15,2"));
        assert_eq!(id.vendor.get().map(String::as_str), Some("Apple Inc."));
        assert_eq!(
            id.uuid.get().map(String::as_str),
            Some("4c4c4544-0042-3510-8051-b4c04f4e4e32")
        );
        assert_eq!(id.board_name.value, None, "binary board-id is not a string");
        assert_eq!(
            id.bios_version.get().map(String::as_str),
            Some("1715.81.2.0.0")
        );
        assert!(id.board_serial.note.is_some());

        let fallback = identity(None, Some(&hardware));
        assert_eq!(
            fallback.model.get().map(String::as_str),
            Some("MacBookPro99,9")
        );
        assert!(fallback.model.source.starts_with("system_profiler"));
    }

    #[test]
    fn apple_silicon_max_capacity_is_a_percentage() {
        let node = plist(
            "<dict>\
             <key>DesignCapacity</key><integer>4382</integer>\
             <key>MaxCapacity</key><integer>100</integer>\
             <key>AppleRawMaxCapacity</key><integer>3901</integer>\
             <key>CycleCount</key><integer>187</integer>\
             <key>Serial</key><string>F8Y0123ABCD</string>\
             <key>ManufactureDate</key><integer>21579</integer>\
             </dict>",
        );
        let b = battery(&node, 0);
        assert_eq!(b.design_capacity.value.unwrap().value, 4382);
        assert_eq!(b.full_charge_capacity.value.unwrap().value, 3901);
        assert_eq!(
            b.full_charge_capacity.value.unwrap().unit,
            CapacityUnit::MilliampHours
        );
        assert_eq!(b.cycle_count.value, Some(187));
        assert_eq!(b.serial.get().map(String::as_str), Some("F8Y0123ABCD"));
        assert_eq!(
            b.manufacture_date.get().map(String::as_str),
            Some("2022-02-11")
        );

        let no_raw = plist(
            "<dict><key>DesignCapacity</key><integer>4382</integer>\
             <key>MaxCapacity</key><integer>100</integer>\
             <key>CycleCount</key><integer>0</integer></dict>",
        );
        let b = battery(&no_raw, 0);
        assert_eq!(b.full_charge_capacity.value, None);
        assert!(b.full_charge_capacity.note.is_some());
        assert_eq!(b.cycle_count.value, None, "zero cycles means not reported");

        let intel = plist(
            "<dict><key>DesignCapacity</key><integer>6669</integer>\
             <key>MaxCapacity</key><integer>5821</integer></dict>",
        );
        assert_eq!(
            battery(&intel, 0).full_charge_capacity.value.unwrap().value,
            5821
        );
    }

    #[test]
    fn drives_come_from_system_profiler_and_are_never_graded() {
        let root = plist(
            "<array><dict><key>_dataType</key><string>SPNVMeDataType</string><key>_items</key><array>\
             <dict><key>_name</key><string>Apple SSD Controller</string><key>_items</key><array>\
             <dict><key>_name</key><string>APPLE SSD AP0512Q</string>\
             <key>bsd_name</key><string>disk0</string>\
             <key>device_model</key><string>APPLE SSD AP0512Q</string>\
             <key>device_serial</key><string>0ba0123456789abc</string>\
             <key>device_revision</key><string>555</string>\
             <key>size_in_bytes</key><integer>500277790720</integer>\
             <key>smart_status</key><string>Verified</string>\
             <key>removable_media</key><string>no</string>\
             </dict></array></dict></array></dict></array>",
        );
        let d = drives(&root);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "disk0");
        assert_eq!(d[0].transport, Transport::Nvme);
        assert_eq!(
            d[0].serial.get().map(String::as_str),
            Some("0ba0123456789abc")
        );
        assert_eq!(d[0].capacity_bytes.value, Some(500_277_790_720));
        match &d[0].health {
            DriveHealth::Unavailable { reason } => assert!(reason.contains("Verified")),
            h => panic!("pass/fail summary must not be graded: {h:?}"),
        }
    }

    #[test]
    fn network_skips_dongles_and_duplicates() {
        let tree = plist(
            "<array>\
             <dict><key>IOMACAddress</key><data>pIOAAQID</data>\
             <key>IORegistryEntryChildren</key><array><dict>\
             <key>BSD Name</key><string>en0</string><key>IOBuiltin</key><true/></dict></array></dict>\
             <dict><key>IOMACAddress</key><data>AOBMaAAB</data>\
             <key>IORegistryEntryChildren</key><array><dict>\
             <key>BSD Name</key><string>en7</string><key>IOBuiltin</key><false/></dict></array></dict>\
             <dict><key>IOMACAddress</key><data>pIOAAQID</data></dict>\
             </array>",
        );
        let n = network(&tree);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].name, "en0");
        assert_eq!(n[0].mac, "a4:83:80:01:02:03");
    }

    #[test]
    fn activation_lock_unknown_is_never_off() {
        let hw = |body: &str| plist(&format!("<dict>{body}</dict>"));
        let on = hw("<key>activation_lock_status</key><string>activation_lock_enabled</string>");
        let off = hw("<key>activation_lock_status</key><string>activation_lock_disabled</string>");
        let m1 = hw("<key>chip_type</key><string>Apple M1 Pro</string>");
        let old = hw("<key>machine_model</key><string>MacBookPro11,1</string>");
        let t2 = vec![plist(
            "<dict><key>ibridge_model_name</key><string>Apple T2 Security Chip</string></dict>",
        )];
        assert_eq!(
            activation_lock(Some(&on), &[]),
            Some(ActivationLock::Enabled)
        );
        assert_eq!(
            activation_lock(Some(&off), &[]),
            Some(ActivationLock::Disabled)
        );
        assert_eq!(activation_lock(Some(&m1), &[]), None);
        assert_eq!(activation_lock(Some(&old), &t2), None);
        assert_eq!(
            activation_lock(Some(&old), &[]),
            Some(ActivationLock::Unsupported)
        );
        assert_eq!(activation_lock(None, &[]), None);
    }

    #[test]
    fn parses_profiles_and_firmwarepasswd() {
        let s =
            parse_profiles_status("Enrolled via DEP: Yes\nMDM enrollment: Yes (User Approved)\n");
        assert_eq!(s.automated, Some(true));
        assert_eq!(s.mdm, Some((true, "Yes (User Approved)".into())));
        let s = parse_profiles_status("Enrolled via DEP: No\nMDM enrollment: No\n");
        assert_eq!(s.automated, Some(false));
        assert_eq!(s.mdm.map(|m| m.0), Some(false));
        assert_eq!(parse_profiles_status("garbage"), EnrolmentStatus::default());

        assert_eq!(parse_firmwarepasswd("Password Enabled: Yes\n"), Some(true));
        assert_eq!(parse_firmwarepasswd("Password Enabled: No\n"), Some(false));
        assert_eq!(parse_firmwarepasswd(""), None);
    }
}
