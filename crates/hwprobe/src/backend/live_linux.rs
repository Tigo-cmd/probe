//! Live readings on Linux, all from sysfs. Each reader takes the sysfs root
//! so it can be tested against a fixture tree.

use std::fs::{self, File};
use std::path::Path;

use crate::live::*;

pub const READ_ROOTS: [&str; 4] = ["/usr/lib", "/usr/share", "/usr/bin", "/opt"];
pub const UNCACHED_METHOD: &str = "page cache dropped per file (posix_fadvise DONTNEED)";

fn read(p: &Path) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn num(p: &Path) -> Option<i64> {
    read(p)?.parse().ok()
}

fn entries(p: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = fs::read_dir(p)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub fn power_now() -> PowerNow {
    power_at(Path::new("/sys"))
}

pub fn power_at(sys: &Path) -> PowerNow {
    let mut p = PowerNow {
        source: "sysfs:/sys/class/power_supply".into(),
        ..PowerNow::default()
    };
    let mut ac_seen = false;
    for dir in entries(&sys.join("class/power_supply")) {
        let kind = read(&dir.join("type")).unwrap_or_default();
        let online = num(&dir.join("online")).map(|v| v == 1);
        match kind.as_str() {
            "Mains" | "USB" => {
                if let Some(o) = online {
                    ac_seen = true;
                    p.on_ac = Some(p.on_ac.unwrap_or(false) || o);
                }
                if online == Some(true) {
                    // USB-C Power Delivery sources publish what they offer.
                    let v = num(&dir.join("voltage_max"));
                    let a = num(&dir.join("current_max"));
                    if let (Some(v), Some(a)) = (v, a) {
                        let w = (v as f64 / 1e6 * a as f64 / 1e6).round() as u32;
                        if w > 0 {
                            p.adapter_watts = Some(p.adapter_watts.unwrap_or(0).max(w));
                        }
                    }
                }
            }
            "Battery" => {
                if read(&dir.join("scope")).as_deref() == Some("Device") {
                    continue; // a wireless mouse or headset
                }
                let status = read(&dir.join("status")).unwrap_or_default();
                p.charging = Some(status == "Charging");
                if !ac_seen && status == "Discharging" {
                    p.on_ac = Some(false);
                }
                p.percent = num(&dir.join("capacity")).map(|c| c as f64);
                // Energy in µWh, or charge in µAh times voltage in µV.
                let uv =
                    num(&dir.join("voltage_now")).or_else(|| num(&dir.join("voltage_min_design")));
                let energy = |e: &str, c: &str| {
                    num(&dir.join(e))
                        .map(|v| (v / 1000) as u64)
                        .or_else(|| Some((num(&dir.join(c))? as f64 * uv? as f64 / 1e9) as u64))
                };
                p.energy_mwh = energy("energy_now", "charge_now");
                p.full_mwh = energy("energy_full", "charge_full");
                let rate = num(&dir.join("power_now")).map(|v| v / 1000).or_else(|| {
                    Some((num(&dir.join("current_now"))? as f64 * uv? as f64 / 1e9) as i64)
                });
                // sysfs reports the magnitude; the status gives the direction.
                p.rate_mw = rate.map(|r| {
                    if status == "Discharging" {
                        -r.abs()
                    } else {
                        r.abs()
                    }
                });
                if p.percent.is_none() {
                    if let (Some(e), Some(f)) = (p.energy_mwh, p.full_mwh) {
                        if f > 0 {
                            p.percent = Some(e as f64 * 100.0 / f as f64);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    p
}

pub fn cpu_now() -> CpuNow {
    cpu_at(Path::new("/sys"))
}

pub fn cpu_at(sys: &Path) -> CpuNow {
    let mut cur = Vec::new();
    let mut max = None;
    for dir in entries(&sys.join("devices/system/cpu")) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.starts_with("cpu")
            || !name[3..].chars().all(|c| c.is_ascii_digit())
            || name.len() == 3
        {
            continue;
        }
        if let Some(khz) = num(&dir.join("cpufreq/scaling_cur_freq")) {
            cur.push(khz);
        }
        if let Some(m) = num(&dir.join("cpufreq/cpuinfo_max_freq")) {
            max = Some(max.unwrap_or(0).max(m));
        }
    }
    let mhz = (!cur.is_empty()).then(|| (cur.iter().sum::<i64>() / cur.len() as i64 / 1000) as u32);
    // Processor sensors first; the ACPI zone is a whole-board fallback.
    let mut temp: Option<f32> = None;
    for hw in entries(&sys.join("class/hwmon")) {
        let name = read(&hw.join("name")).unwrap_or_default();
        if [
            "coretemp",
            "k10temp",
            "zenpower",
            "cpu_thermal",
            "soc_thermal",
        ]
        .contains(&name.as_str())
        {
            for f in entries(&hw) {
                let n = f
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if n.starts_with("temp") && n.ends_with("_input") {
                    if let Some(m) = num(&f) {
                        let c = m as f32 / 1000.0;
                        if (1.0..130.0).contains(&c) {
                            temp = Some(temp.map_or(c, |t: f32| t.max(c)));
                        }
                    }
                }
            }
        }
    }
    if temp.is_none() {
        for z in entries(&sys.join("class/thermal")) {
            let kind = read(&z.join("type")).unwrap_or_default();
            if ["x86_pkg_temp", "cpu-thermal", "cpu_thermal", "acpitz"].contains(&kind.as_str()) {
                if let Some(m) = num(&z.join("temp")) {
                    let c = m as f32 / 1000.0;
                    if (1.0..130.0).contains(&c) {
                        temp = Some(temp.map_or(c, |t: f32| t.max(c)));
                    }
                }
            }
        }
    }
    CpuNow {
        mhz,
        max_mhz: max.map(|m| (m / 1000) as u32),
        temp_c: temp,
        limit_pct: None,
        source: "sysfs:cpufreq, hwmon, thermal".into(),
    }
}

pub fn radios() -> Vec<Radio> {
    radios_at(Path::new("/sys"))
}

pub fn radios_at(sys: &Path) -> Vec<Radio> {
    // rfkill says whether a switch or airplane mode has the radio blocked.
    let mut blocked = std::collections::HashMap::new();
    for r in entries(&sys.join("class/rfkill")) {
        let kind = read(&r.join("type")).unwrap_or_default();
        let off = num(&r.join("soft")) == Some(1) || num(&r.join("hard")) == Some(1);
        let e = blocked.entry(kind).or_insert(true);
        *e = *e && off;
    }
    let mut out = Vec::new();
    for net in entries(&sys.join("class/net")) {
        if net.join("wireless").exists() || net.join("phy80211").exists() {
            let name = net
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let up = read(&net.join("operstate")).map(|s| s == "up");
            out.push(Radio {
                kind: RadioKind::Wifi,
                name,
                enabled: blocked.get("wlan").map(|b| !b).or(Some(true)),
                connected: up,
            });
        }
    }
    for hci in entries(&sys.join("class/bluetooth")) {
        let name = hci
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.starts_with("hci") || name.contains(':') {
            continue;
        }
        out.push(Radio {
            kind: RadioKind::Bluetooth,
            name,
            enabled: blocked.get("bluetooth").map(|b| !b),
            connected: None,
        });
    }
    out
}

pub fn usb_devices() -> Vec<UsbDevice> {
    usb_at(Path::new("/sys"))
}

pub fn usb_at(sys: &Path) -> Vec<UsbDevice> {
    let mut out = Vec::new();
    for dev in entries(&sys.join("bus/usb/devices")) {
        let id = dev
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Interfaces carry a colon; root hubs are named usbN.
        if id.contains(':') || id.starts_with("usb") {
            continue;
        }
        let Some(vendor) = read(&dev.join("idVendor")) else {
            continue;
        };
        let product_id = read(&dev.join("idProduct")).unwrap_or_default();
        let name = match (read(&dev.join("manufacturer")), read(&dev.join("product"))) {
            (Some(m), Some(p)) if !p.starts_with(&m) => format!("{m} {p}"),
            (_, Some(p)) => p,
            (Some(m), None) => m,
            (None, None) => format!("USB device {vendor}:{product_id}"),
        };
        out.push(UsbDevice {
            id: format!("{id}/{vendor}:{product_id}"),
            name,
        });
    }
    out
}

pub fn open_uncached(path: &Path) -> std::io::Result<File> {
    use std::os::unix::io::AsRawFd;
    let f = File::open(path)?;
    // SAFETY: fd is a valid open descriptor; DONTNEED only drops clean
    // cached pages of this file, so the following read comes from the drive.
    unsafe {
        libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
        libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_SEQUENTIAL);
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(std::path::PathBuf);
    impl Tree {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("live-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Tree(root)
        }
        fn put(&self, rel: &str, v: &str) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, v).unwrap();
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_charging_state_rate_and_usb_c_wattage() {
        let t = Tree::new("power");
        t.put("class/power_supply/AC/type", "Mains\n");
        t.put("class/power_supply/AC/online", "0\n");
        t.put(
            "class/power_supply/ucsi-source-psy-USBC000:001/type",
            "USB\n",
        );
        t.put(
            "class/power_supply/ucsi-source-psy-USBC000:001/online",
            "1\n",
        );
        t.put(
            "class/power_supply/ucsi-source-psy-USBC000:001/voltage_max",
            "20000000\n",
        );
        t.put(
            "class/power_supply/ucsi-source-psy-USBC000:001/current_max",
            "3250000\n",
        );
        t.put("class/power_supply/BAT0/type", "Battery\n");
        t.put("class/power_supply/BAT0/status", "Charging\n");
        t.put("class/power_supply/BAT0/capacity", "57\n");
        t.put("class/power_supply/BAT0/energy_now", "29000000\n");
        t.put("class/power_supply/BAT0/energy_full", "51000000\n");
        t.put("class/power_supply/BAT0/power_now", "31500000\n");
        t.put("class/power_supply/hid-mouse/type", "Battery\n");
        t.put("class/power_supply/hid-mouse/scope", "Device\n");
        t.put("class/power_supply/hid-mouse/status", "Discharging\n");
        let p = power_at(&t.0);
        assert_eq!(p.on_ac, Some(true));
        assert_eq!(p.charging, Some(true));
        assert_eq!(p.percent, Some(57.0));
        assert_eq!((p.energy_mwh, p.full_mwh), (Some(29_000), Some(51_000)));
        assert_eq!(p.rate_mw, Some(31_500));
        assert_eq!(p.adapter_watts, Some(65));
    }

    #[test]
    fn converts_charge_to_energy_and_signs_discharge() {
        let t = Tree::new("charge");
        t.put("class/power_supply/BAT1/type", "Battery\n");
        t.put("class/power_supply/BAT1/status", "Discharging\n");
        t.put("class/power_supply/BAT1/charge_now", "2000000\n");
        t.put("class/power_supply/BAT1/charge_full", "4000000\n");
        t.put("class/power_supply/BAT1/current_now", "1000000\n");
        t.put("class/power_supply/BAT1/voltage_now", "12000000\n");
        let p = power_at(&t.0);
        assert_eq!(p.on_ac, Some(false));
        assert_eq!(p.energy_mwh, Some(24_000));
        assert_eq!(p.rate_mw, Some(-12_000));
        assert_eq!(p.percent, Some(50.0));
    }

    #[test]
    fn reads_clock_and_the_processor_sensor() {
        let t = Tree::new("cpu");
        for (i, khz) in [(0, "2400000"), (1, "1600000")] {
            t.put(
                &format!("devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq"),
                khz,
            );
            t.put(
                &format!("devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq"),
                "4200000",
            );
        }
        t.put("devices/system/cpu/cpufreq/policy0/scaling_cur_freq", "1");
        t.put("class/hwmon/hwmon0/name", "acpitz\n");
        t.put("class/hwmon/hwmon0/temp1_input", "99000\n");
        t.put("class/hwmon/hwmon1/name", "coretemp\n");
        t.put("class/hwmon/hwmon1/temp1_input", "61000\n");
        t.put("class/hwmon/hwmon1/temp2_input", "67500\n");
        let c = cpu_at(&t.0);
        assert_eq!((c.mhz, c.max_mhz), (Some(2000), Some(4200)));
        assert_eq!(c.temp_c, Some(67.5), "the processor sensor, not the board");
    }

    #[test]
    fn finds_radios_and_usb_devices() {
        let t = Tree::new("radio");
        t.put("class/net/wlp2s0/wireless/x", "");
        t.put("class/net/wlp2s0/operstate", "up\n");
        t.put("class/net/eth0/operstate", "up\n");
        t.put("class/bluetooth/hci0/x", "");
        t.put("class/rfkill/rfkill1/type", "bluetooth\n");
        t.put("class/rfkill/rfkill1/soft", "1\n");
        t.put("bus/usb/devices/usb1/idVendor", "1d6b\n");
        t.put("bus/usb/devices/1-2/idVendor", "0781\n");
        t.put("bus/usb/devices/1-2/idProduct", "5583\n");
        t.put("bus/usb/devices/1-2/manufacturer", "SanDisk\n");
        t.put("bus/usb/devices/1-2/product", "Ultra Fit\n");
        t.put("bus/usb/devices/1-2:1.0/idVendor", "0781\n");
        let r = radios_at(&t.0);
        assert_eq!(r.len(), 2);
        assert_eq!(
            (r[0].kind, r[0].connected, r[0].enabled),
            (RadioKind::Wifi, Some(true), Some(true))
        );
        assert_eq!(
            (r[1].kind, r[1].enabled),
            (RadioKind::Bluetooth, Some(false))
        );
        let u = usb_at(&t.0);
        assert_eq!(
            u,
            [UsbDevice {
                id: "1-2/0781:5583".into(),
                name: "SanDisk Ultra Fit".into()
            }]
        );
    }
}
