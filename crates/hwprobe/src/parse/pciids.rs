//! Lookups in the `pci.ids` database that most Linux systems ship
//! (`/usr/share/hwdata/pci.ids` or `/usr/share/misc/pci.ids`).
//!
//! The format is line based: a vendor line starts in column 0 with four hex
//! digits, its device lines start with one tab, subsystem lines with two.
//! The file is untrusted text; malformed lines are skipped.

/// The vendor and device names for `vendor:device`, as far as they are listed.
pub fn lookup(text: &str, vendor: u16, device: u16) -> (Option<String>, Option<String>) {
    let vendor_hex = format!("{vendor:04x}");
    let device_hex = format!("{device:04x}");
    let mut vendor_name = None;
    let mut in_vendor = false;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('\t') {
            if !in_vendor || rest.starts_with('\t') {
                continue;
            }
            if let Some(name) = entry(rest, &device_hex) {
                return (vendor_name, Some(name));
            }
        } else {
            // A new top-level entry. The class list at the end ("C 03 ...")
            // never matches four hex digits followed by spaces.
            if in_vendor {
                break;
            }
            if let Some(name) = entry(line, &vendor_hex) {
                vendor_name = Some(name);
                in_vendor = true;
            }
        }
    }
    (vendor_name, None)
}

/// `"10de  NVIDIA Corporation"` → `Some("NVIDIA Corporation")` when the id matches.
fn entry(line: &str, id: &str) -> Option<String> {
    let (key, name) = line.split_once(char::is_whitespace)?;
    (key.eq_ignore_ascii_case(id))
        .then(|| name.trim().to_string())
        .filter(|n| !n.is_empty())
}

/// The marketing name from a device entry: the bracketed part when present
/// (`"GA107M [GeForce RTX 3050 Mobile]"` → `"GeForce RTX 3050 Mobile"`).
pub fn marketing_name(device: &str) -> &str {
    match (device.rfind('['), device.rfind(']')) {
        (Some(open), Some(close)) if close > open + 1 => device[open + 1..close].trim(),
        _ => device.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: &str = "\
# comment
10de  NVIDIA Corporation
\t1f99  TU117M [GeForce GTX 1650 Mobile / Max-Q]
\t25a2  GA107M [GeForce RTX 3050 Mobile]
\t\t17aa 3a5e  some subsystem
8086  Intel Corporation
\t9a49  TigerLake-LP GT2 [Iris Xe Graphics]
\t5917  UHD Graphics 620
C 03  Display controller
";

    #[test]
    fn finds_vendor_and_device() {
        let (v, d) = lookup(IDS, 0x10de, 0x25a2);
        assert_eq!(v.as_deref(), Some("NVIDIA Corporation"));
        assert_eq!(d.as_deref(), Some("GA107M [GeForce RTX 3050 Mobile]"));
        assert_eq!(marketing_name(&d.unwrap()), "GeForce RTX 3050 Mobile");
        let (_, d) = lookup(IDS, 0x8086, 0x5917);
        assert_eq!(marketing_name(&d.unwrap()), "UHD Graphics 620");
    }

    #[test]
    fn unknown_ids_and_junk_do_not_panic() {
        assert_eq!(lookup(IDS, 0x1002, 0x1234), (None, None));
        let (v, d) = lookup(IDS, 0x8086, 0xffff);
        assert!(v.is_some() && d.is_none());
        for junk in ["", "\t", "\t\t", "zzzz", "[", "]", "\u{0}\t\u{0}"] {
            let _ = lookup(junk, 0, 0);
            let _ = marketing_name(junk);
        }
        assert_eq!(marketing_name("odd ] name ["), "odd ] name [");
    }
}
