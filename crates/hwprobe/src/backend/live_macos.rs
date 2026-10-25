//! Live readings on macOS from tools that ship with it: `ioreg -a`,
//! `pmset -g therm` and `system_profiler -xml`. The parsing here is pure and
//! tested on every platform; only `sys` runs commands.

use crate::live::*;
use crate::parse::plist::Value;

fn int(v: Option<&Value>) -> Option<i128> {
    v.and_then(Value::as_int)
}

/// The I/O Registry prints signed counters as unsigned 64-bit numbers.
fn signed(v: i128) -> i64 {
    if v > i64::MAX as i128 {
        (v - (1i128 << 64)) as i64
    } else {
        v as i64
    }
}

/// Power from the first `AppleSmartBattery` node of `ioreg -a -r -c AppleSmartBattery`.
pub fn power_from_ioreg(tree: &Value) -> PowerNow {
    let mut p = PowerNow {
        source: "ioreg:AppleSmartBattery".into(),
        ..PowerNow::default()
    };
    let Some(b) = tree
        .as_array()
        .first()
        .or(Some(tree))
        .filter(|b| b.get("Voltage").is_some() || b.get("ExternalConnected").is_some())
    else {
        return p;
    };
    p.on_ac = b.get("ExternalConnected").and_then(Value::as_bool);
    p.charging = b.get("IsCharging").and_then(Value::as_bool);
    let cur = int(b.get("CurrentCapacity"));
    let max = int(b.get("MaxCapacity"));
    p.percent = match (cur, max) {
        // Apple silicon reports both as percentages.
        (Some(c), Some(m)) if m <= 100 => Some(c as f64),
        (Some(c), Some(m)) if m > 0 => Some(c as f64 * 100.0 / m as f64),
        _ => None,
    };
    let mv = int(b.get("Voltage")).filter(|&v| v > 0);
    let mah = |k: &str| int(b.get(k)).filter(|&v| v > 0);
    let raw_cur = mah("AppleRawCurrentCapacity").or(cur.filter(|_| max.is_some_and(|m| m > 100)));
    let raw_max = mah("AppleRawMaxCapacity").or(max.filter(|&m| m > 100));
    if let Some(mv) = mv {
        p.energy_mwh = raw_cur.map(|c| (c * mv / 1000) as u64);
        p.full_mwh = raw_max.map(|c| (c * mv / 1000) as u64);
        let ma = int(b.get("InstantAmperage"))
            .or(int(b.get("Amperage")))
            .map(signed);
        p.rate_mw = ma.map(|a| a * mv as i64 / 1000);
    }
    p.adapter_watts = b
        .get("AdapterDetails")
        .and_then(|d| int(d.get("Watts")))
        .filter(|&w| w > 0 && w < 1000)
        .map(|w| w as u32);
    p
}

/// `CPU_Speed_Limit = 100` from `pmset -g therm` (Intel Macs report it).
pub fn speed_limit_from_pmset(text: &str) -> Option<u8> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("CPU_Speed_Limit"))
        .and_then(|rest| {
            rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace())
                .parse::<u8>()
                .ok()
        })
        .filter(|&v| v <= 100)
}

fn profiler_items<'a>(root: &'a Value, data_type: &str) -> &'a [Value] {
    root.as_array()
        .iter()
        .find(|d| d.get("_dataType").and_then(Value::as_str) == Some(data_type))
        .and_then(|d| d.get("_items"))
        .map(Value::as_array)
        .unwrap_or(&[])
}

/// Wi-Fi and Bluetooth from `system_profiler -xml SPAirPortDataType SPBluetoothDataType`.
pub fn radios_from_profiler(root: &Value) -> Vec<Radio> {
    let mut out = Vec::new();
    for item in profiler_items(root, "SPAirPortDataType") {
        for iface in item
            .get("spairport_airport_interfaces")
            .map(Value::as_array)
            .unwrap_or(&[])
        {
            let status = iface
                .get("spairport_status_information")
                .and_then(Value::text)
                .unwrap_or_default();
            let connected = iface.get("spairport_current_network_information").is_some()
                || status.contains("connected");
            out.push(Radio {
                kind: RadioKind::Wifi,
                name: iface
                    .get("_name")
                    .and_then(Value::text)
                    .unwrap_or_else(|| "Wi-Fi".into()),
                enabled: (!status.is_empty()).then(|| !status.contains("off")),
                connected: Some(connected),
            });
        }
    }
    for item in profiler_items(root, "SPBluetoothDataType") {
        let props = item
            .get("controller_properties")
            .or_else(|| item.get("local_device_title"));
        let Some(props) = props else { continue };
        let state = props
            .get("controller_state")
            .or_else(|| props.get("general_power"))
            .and_then(Value::text)
            .unwrap_or_default();
        let chip = props
            .get("controller_chipset")
            .and_then(Value::text)
            .map(|c| format!("Bluetooth ({c})"))
            .unwrap_or_else(|| "Bluetooth".into());
        out.push(Radio {
            kind: RadioKind::Bluetooth,
            name: chip,
            enabled: (!state.is_empty()).then(|| state.ends_with("on")),
            connected: None,
        });
    }
    out
}

/// USB devices from `ioreg -a -r -c IOUSBHostDevice`.
pub fn usb_from_ioreg(tree: &Value) -> Vec<UsbDevice> {
    tree.as_array()
        .iter()
        .filter_map(|d| {
            let vendor = int(d.get("idVendor"))?;
            let product = int(d.get("idProduct")).unwrap_or(0);
            let location = int(d.get("locationID")).unwrap_or(0);
            let text = |k: &str| {
                d.get(k)
                    .and_then(Value::text)
                    .filter(|s| !s.trim().is_empty())
            };
            let name = match (
                text("USB Vendor Name"),
                text("USB Product Name").or_else(|| text("kUSBProductString")),
            ) {
                (Some(v), Some(p)) if !p.starts_with(&v) => format!("{v} {p}"),
                (_, Some(p)) => p,
                (Some(v), None) => v,
                (None, None) => format!("USB device {vendor:04x}:{product:04x}"),
            };
            Some(UsbDevice {
                id: format!("{location:08x}/{vendor:04x}:{product:04x}"),
                name,
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub mod sys {
    use std::fs::File;
    use std::path::Path;
    use std::process::Command;

    use super::*;
    use crate::parse::plist;

    pub const READ_ROOTS: [&str; 4] = [
        "/System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld",
        "/System/Library/dyld",
        "/System/Library/PrivateFrameworks",
        "/Applications",
    ];
    pub const UNCACHED_METHOD: &str = "cache bypassed per file (F_NOCACHE)";

    fn run(cmd: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(cmd).args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn plist_of(cmd: &str, args: &[&str]) -> Option<Value> {
        plist::parse(&run(cmd, args)?).ok()
    }

    pub fn power_now() -> PowerNow {
        plist_of("/usr/sbin/ioreg", &["-a", "-r", "-c", "AppleSmartBattery"])
            .map(|t| power_from_ioreg(&t))
            .unwrap_or_default()
    }

    pub fn cpu_now() -> CpuNow {
        let limit =
            run("/usr/bin/pmset", &["-g", "therm"]).and_then(|t| speed_limit_from_pmset(&t));
        let hz = run("/usr/sbin/sysctl", &["-n", "hw.cpufrequency"])
            .and_then(|t| t.trim().parse::<u64>().ok());
        CpuNow {
            mhz: None,
            max_mhz: hz.map(|h| (h / 1_000_000) as u32),
            temp_c: None,
            limit_pct: limit,
            source: "pmset -g therm, sysctl".into(),
        }
    }

    pub fn radios() -> Vec<Radio> {
        plist_of(
            "/usr/sbin/system_profiler",
            &["-xml", "SPAirPortDataType", "SPBluetoothDataType"],
        )
        .map(|t| radios_from_profiler(&t))
        .unwrap_or_default()
    }

    pub fn usb_devices() -> Vec<UsbDevice> {
        plist_of("/usr/sbin/ioreg", &["-a", "-r", "-c", "IOUSBHostDevice"])
            .map(|t| usb_from_ioreg(&t))
            .unwrap_or_default()
    }

    pub fn open_uncached(path: &Path) -> std::io::Result<File> {
        use std::os::unix::io::AsRawFd;
        let f = File::open(path)?;
        // SAFETY: fd is valid; F_NOCACHE only changes caching for this descriptor.
        unsafe { libc::fcntl(f.as_raw_fd(), libc::F_NOCACHE, 1) };
        Ok(f)
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
    fn reads_apple_silicon_power_with_negative_current() {
        let t = plist(
            "<array><dict>\
             <key>ExternalConnected</key><false/><key>IsCharging</key><false/>\
             <key>CurrentCapacity</key><integer>82</integer><key>MaxCapacity</key><integer>100</integer>\
             <key>AppleRawCurrentCapacity</key><integer>4100</integer><key>AppleRawMaxCapacity</key><integer>5000</integer>\
             <key>Voltage</key><integer>12000</integer>\
             <key>InstantAmperage</key><integer>18446744073709550616</integer>\
             </dict></array>",
        );
        let p = power_from_ioreg(&t);
        assert_eq!(
            (p.on_ac, p.charging, p.percent),
            (Some(false), Some(false), Some(82.0))
        );
        assert_eq!((p.energy_mwh, p.full_mwh), (Some(49_200), Some(60_000)));
        assert_eq!(p.rate_mw, Some(-12_000), "-1000 mA at 12 V");
    }

    #[test]
    fn reads_adapter_watts_and_an_intel_style_capacity() {
        let t = plist(
            "<array><dict>\
             <key>ExternalConnected</key><true/><key>IsCharging</key><true/>\
             <key>CurrentCapacity</key><integer>3000</integer><key>MaxCapacity</key><integer>6000</integer>\
             <key>Voltage</key><integer>12500</integer><key>Amperage</key><integer>2000</integer>\
             <key>AdapterDetails</key><dict><key>Watts</key><integer>96</integer></dict>\
             </dict></array>",
        );
        let p = power_from_ioreg(&t);
        assert_eq!(p.percent, Some(50.0));
        assert_eq!(p.rate_mw, Some(25_000));
        assert_eq!(p.adapter_watts, Some(96));
        assert_eq!(
            power_from_ioreg(&plist("<array></array>")),
            PowerNow {
                source: "ioreg:AppleSmartBattery".into(),
                ..PowerNow::default()
            }
        );
    }

    #[test]
    fn reads_the_speed_limit() {
        assert_eq!(
            speed_limit_from_pmset(
                "Note: No thermal warning level has been recorded\nCPU_Speed_Limit \t= 73\n"
            ),
            Some(73)
        );
        assert_eq!(speed_limit_from_pmset("No thermal warning"), None);
    }

    #[test]
    fn reads_radios_and_usb() {
        let r = plist(
            "<array>\
             <dict><key>_dataType</key><string>SPAirPortDataType</string><key>_items</key><array><dict>\
               <key>spairport_airport_interfaces</key><array><dict><key>_name</key><string>en0</string>\
               <key>spairport_status_information</key><string>spairport_status_connected</string></dict></array>\
             </dict></array></dict>\
             <dict><key>_dataType</key><string>SPBluetoothDataType</string><key>_items</key><array><dict>\
               <key>controller_properties</key><dict><key>controller_state</key><string>attrib_off</string>\
               <key>controller_chipset</key><string>BCM_4387</string></dict>\
             </dict></array></dict></array>",
        );
        let radios = radios_from_profiler(&r);
        assert_eq!(radios.len(), 2);
        assert_eq!(
            (radios[0].enabled, radios[0].connected),
            (Some(true), Some(true))
        );
        assert_eq!(radios[1].enabled, Some(false));
        assert_eq!(radios[1].name, "Bluetooth (BCM_4387)");

        let u = plist(
            "<array><dict><key>idVendor</key><integer>1921</integer><key>idProduct</key><integer>21891</integer>\
             <key>locationID</key><integer>17825792</integer><key>USB Product Name</key><string>Ultra Fit</string>\
             <key>USB Vendor Name</key><string>SanDisk</string></dict></array>",
        );
        assert_eq!(
            usb_from_ioreg(&u),
            [UsbDevice {
                id: "01100000/0781:5583".into(),
                name: "SanDisk Ultra Fit".into()
            }]
        );
    }
}
