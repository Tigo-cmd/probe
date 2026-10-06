//! Per-OS extraction backends.
//!
//! Each backend reads what it can at the current privilege level and records
//! every failed read as a [`ProbeNote`](crate::model::ProbeNote). Backends do
//! not grade: they only extract and tag provenance.

#[cfg(target_os = "linux")]
pub mod linux;
pub mod signals;
#[cfg(windows)]
pub mod windows;

use crate::model::{EncumbranceSignal, ProbeNote, Scan};

// Used only by the Linux and Windows backends until the macOS backend lands.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
/// Placeholder strings that OEMs leave in SMBIOS fields. These carry no
/// information and must not be treated as identifiers.
const PLACEHOLDERS: &[&str] = &[
    "to be filled by o.e.m.",
    "default string",
    "system serial number",
    "system product name",
    "not specified",
    "not applicable",
    "none",
    "n/a",
    "0123456789",
    "123456789",
    "00000000",
    "0",
    "invalid",
    "chassis serial number",
    "base board serial number",
    "asset tag",
    "no asset tag",
    "03000200-0400-0500-0006-000700080009",
    "00000000-0000-0000-0000-000000000000",
    "ffffffff-ffff-ffff-ffff-ffffffffffff",
];

#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) fn is_placeholder(s: &str) -> bool {
    let t = s.trim().to_ascii_lowercase();
    t.is_empty() || PLACEHOLDERS.contains(&t.as_str()) || t.chars().all(|c| c == 'x' || c == '0')
}

/// Turn a raw WPBT read into a signal. `Ok(None)` means the firmware
/// publishes no WPBT. A failed read yields a note and no signal.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) fn wpbt_signal(
    read: Result<Option<Vec<u8>>, String>,
    source: &str,
    notes: &mut Vec<ProbeNote>,
) -> Option<EncumbranceSignal> {
    match read {
        Ok(None) => Some(signals::wpbt(None, source)),
        Ok(Some(bytes)) => {
            let table = crate::parse::acpi::parse_wpbt(&bytes).unwrap_or_else(|e| {
                notes.push(ProbeNote {
                    component: "encumbrance/wpbt".into(),
                    message: format!("WPBT present but rejected: {e}"),
                });
                crate::parse::acpi::Wpbt {
                    oem_id: "unparsed table".into(),
                    ..Default::default()
                }
            });
            Some(signals::wpbt(Some(&table), source))
        }
        Err(message) => {
            notes.push(ProbeNote {
                component: "encumbrance/wpbt".into(),
                message,
            });
            None
        }
    }
}

/// Run the backend for the current operating system.
pub fn scan() -> Scan {
    #[cfg(target_os = "linux")]
    {
        linux::LinuxBackend::system().scan()
    }
    #[cfg(windows)]
    {
        windows::WindowsBackend::system().scan()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        unsupported()
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn unsupported() -> Scan {
    use crate::model::SCAN_SCHEMA_VERSION;
    Scan {
        schema_version: SCAN_SCHEMA_VERSION,
        tool_version: crate::VERSION.to_string(),
        os: std::env::consts::OS.to_string(),
        started_at: crate::unix_now(),
        probe_notes: vec![ProbeNote {
            component: "backend".into(),
            message: format!("no extraction backend for {} yet", std::env::consts::OS),
        }],
        ..Scan::default()
    }
}

#[cfg(test)]
mod tests {
    use super::is_placeholder;

    #[test]
    fn recognises_oem_placeholders() {
        for s in [
            "To Be Filled By O.E.M.",
            "Default string",
            "  ",
            "XXXXXXXX",
            "0000",
            "None",
        ] {
            assert!(is_placeholder(s), "{s:?}");
        }
        for s in ["PF0ABCDE", "20AQ0069UK", "LENOVO"] {
            assert!(!is_placeholder(s), "{s:?}");
        }
    }
}
