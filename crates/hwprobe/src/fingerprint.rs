//! Composite device fingerprint.
//!
//! Any single identifier can be swapped or spoofed, so a scan is keyed on
//! several: SMBIOS UUID, board serial, drive serials and MAC addresses.
//! Divergence between two scans of "the same" machine is the tamper signal.

use serde::{Deserialize, Serialize};

use crate::model::Scan;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub smbios_uuid: Option<String>,
    pub board_serial: Option<String>,
    pub system_serial: Option<String>,
    pub drive_serials: Vec<String>,
    pub battery_serials: Vec<String>,
    pub macs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergence {
    pub component: String,
    pub before: String,
    pub after: String,
}

impl Fingerprint {
    pub fn of(scan: &Scan) -> Self {
        let mut drive_serials: Vec<String> = scan
            .storage
            .iter()
            .filter_map(|d| d.serial.value.clone())
            .collect();
        let mut battery_serials: Vec<String> = scan
            .batteries
            .iter()
            .filter_map(|b| b.serial.value.clone())
            .collect();
        let mut macs: Vec<String> = scan
            .network
            .iter()
            .map(|n| n.mac.to_ascii_lowercase())
            .collect();
        drive_serials.sort();
        battery_serials.sort();
        macs.sort();
        Fingerprint {
            smbios_uuid: scan
                .identity
                .uuid
                .value
                .as_ref()
                .map(|u| u.to_ascii_lowercase()),
            board_serial: scan.identity.board_serial.value.clone(),
            system_serial: scan.identity.serial.value.clone(),
            drive_serials,
            battery_serials,
            macs,
        }
    }

    /// Number of identifiers both fingerprints carry that match.
    pub fn matching(&self, other: &Fingerprint) -> usize {
        let opt = |a: &Option<String>, b: &Option<String>| {
            matches!((a, b), (Some(x), Some(y)) if x == y) as usize
        };
        let shared = |a: &[String], b: &[String]| a.iter().filter(|x| b.contains(x)).count();
        opt(&self.smbios_uuid, &other.smbios_uuid)
            + opt(&self.board_serial, &other.board_serial)
            + opt(&self.system_serial, &other.system_serial)
            + shared(&self.drive_serials, &other.drive_serials)
            + shared(&self.battery_serials, &other.battery_serials)
            + shared(&self.macs, &other.macs)
    }

    /// Identifiers that changed between two scans. A missing value on either
    /// side is not reported as a change: it is absent evidence, not a swap.
    pub fn diverges_from(&self, later: &Fingerprint) -> Vec<Divergence> {
        let mut out = Vec::new();
        let mut opt = |name: &str, a: &Option<String>, b: &Option<String>| {
            if let (Some(x), Some(y)) = (a, b) {
                if x != y {
                    out.push(Divergence {
                        component: name.into(),
                        before: x.clone(),
                        after: y.clone(),
                    });
                }
            }
        };
        opt("smbios_uuid", &self.smbios_uuid, &later.smbios_uuid);
        opt("board_serial", &self.board_serial, &later.board_serial);
        opt("system_serial", &self.system_serial, &later.system_serial);

        let mut set = |name: &str, a: &[String], b: &[String]| {
            if a.is_empty() || b.is_empty() {
                return;
            }
            for x in a.iter().filter(|x| !b.contains(x)) {
                out.push(Divergence {
                    component: name.into(),
                    before: x.clone(),
                    after: String::new(),
                });
            }
            for y in b.iter().filter(|y| !a.contains(y)) {
                out.push(Divergence {
                    component: name.into(),
                    before: String::new(),
                    after: y.clone(),
                });
            }
        };
        set("drive_serial", &self.drive_serials, &later.drive_serials);
        set(
            "battery_serial",
            &self.battery_serials,
            &later.battery_serials,
        );
        set("mac", &self.macs, &later.macs);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(uuid: &str, drive: &str) -> Fingerprint {
        Fingerprint {
            smbios_uuid: Some(uuid.into()),
            board_serial: Some("L1HF4AB0001".into()),
            drive_serials: vec![drive.into()],
            macs: vec!["28:d2:44:00:00:01".into()],
            ..Fingerprint::default()
        }
    }

    #[test]
    fn detects_swapped_drive() {
        let before = fp("4c4c4544-0001", "S3Z9NB0K123");
        let after = fp("4c4c4544-0001", "WD-WX11A0000");
        let d = before.diverges_from(&after);
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|x| x.component == "drive_serial"));
        assert_eq!(before.matching(&after), 3);
    }

    #[test]
    fn missing_values_are_not_divergence() {
        let before = fp("4c4c4544-0001", "S3Z9NB0K123");
        let mut after = before.clone();
        after.board_serial = None;
        after.drive_serials.clear();
        assert!(before.diverges_from(&after).is_empty());
    }
}
