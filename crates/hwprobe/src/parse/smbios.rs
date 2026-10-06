//! SMBIOS structure table: the identity strings in types 0 (BIOS), 1 (system),
//! 2 (baseboard) and 3 (chassis).
//!
//! Windows hands the table over through `GetSystemFirmwareTable('RSMB')`
//! prefixed with an 8-byte `RawSMBIOSData` header; [`parse_raw_smbios_data`]
//! takes that form. Strings are returned as found: the backend decides which
//! are OEM placeholders.

use super::{require_len, ParseError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmbiosIdentity {
    pub version: (u8, u8),
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub bios_date: Option<String>,
    pub sys_vendor: Option<String>,
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub product_serial: Option<String>,
    /// Formatted like Linux `product_uuid`, so scans from either OS compare.
    pub product_uuid: Option<String>,
    pub board_vendor: Option<String>,
    pub board_name: Option<String>,
    pub board_serial: Option<String>,
    pub chassis_type: Option<u32>,
    pub chassis_asset_tag: Option<String>,
}

const RAW_HEADER_LEN: usize = 8;

/// Parse the `RawSMBIOSData` blob returned by `GetSystemFirmwareTable('RSMB', 0)`.
pub fn parse_raw_smbios_data(buf: &[u8]) -> Result<SmbiosIdentity, ParseError> {
    require_len(buf, RAW_HEADER_LEN)?;
    let version = (buf[1], buf[2]);
    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    let end = RAW_HEADER_LEN.saturating_add(len).min(buf.len());
    Ok(parse_table(&buf[RAW_HEADER_LEN..end], version))
}

/// Parse a bare structure table. Truncated or malformed structures end the
/// walk; whatever was read before them is kept.
pub fn parse_table(table: &[u8], version: (u8, u8)) -> SmbiosIdentity {
    let mut id = SmbiosIdentity {
        version,
        ..SmbiosIdentity::default()
    };
    let mut seen = [false; 4];
    for s in structures(table) {
        let t = s.kind as usize;
        if t >= seen.len() || seen[t] {
            continue;
        }
        seen[t] = true;
        match s.kind {
            0 => {
                id.bios_vendor = s.string(0x04);
                id.bios_version = s.string(0x05);
                id.bios_date = s.string(0x08);
            }
            1 => {
                id.sys_vendor = s.string(0x04);
                id.product_name = s.string(0x05);
                id.product_version = s.string(0x06);
                id.product_serial = s.string(0x07);
                id.product_uuid = s
                    .formatted
                    .get(0x08..0x18)
                    .and_then(|b| format_uuid(b, version));
            }
            2 => {
                id.board_vendor = s.string(0x04);
                id.board_name = s.string(0x05);
                id.board_serial = s.string(0x07);
            }
            3 => {
                id.chassis_type = s.formatted.get(0x05).map(|b| (b & 0x7F) as u32);
                id.chassis_asset_tag = s.string(0x08);
            }
            _ => {}
        }
    }
    id
}

struct Structure<'a> {
    kind: u8,
    formatted: &'a [u8],
    strings: &'a [u8],
}

impl Structure<'_> {
    /// The string referenced by the index byte at `offset`; index 0 means none.
    fn string(&self, offset: usize) -> Option<String> {
        let index = *self.formatted.get(offset)? as usize;
        if index == 0 {
            return None;
        }
        let raw = self.strings.split(|&b| b == 0).nth(index - 1)?;
        let s = String::from_utf8_lossy(raw).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn structures(table: &[u8]) -> impl Iterator<Item = Structure<'_>> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let kind = *table.get(at)?;
        let len = *table.get(at + 1)? as usize;
        if len < 4 || kind == 127 {
            return None;
        }
        let formatted = table.get(at..at + len)?;
        // The string set ends with a double NUL; with no strings it is just "\0\0".
        let rest = table.get(at + len..)?;
        let end = rest.windows(2).position(|w| w == [0, 0])?;
        let strings = &rest[..end];
        at += len + end + 2;
        Some(Structure {
            kind,
            formatted,
            strings,
        })
    })
}

fn format_uuid(b: &[u8], version: (u8, u8)) -> Option<String> {
    if b.len() != 16 || b.iter().all(|&x| x == 0) || b.iter().all(|&x| x == 0xFF) {
        return None; // "not present" and "not set" per the specification
    }
    // From SMBIOS 2.6 the first three fields are little-endian.
    let order: [usize; 16] = if version >= (2, 6) {
        [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15]
    } else {
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    };
    let mut s = String::with_capacity(36);
    for (i, &o) in order.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        s.push_str(&format!("{:02x}", b[o]));
    }
    Some(s)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn structure(kind: u8, formatted_tail: &[u8], strings: &[&str]) -> Vec<u8> {
        let mut v = vec![kind, (4 + formatted_tail.len()) as u8, 0, 0];
        v.extend_from_slice(formatted_tail);
        for s in strings {
            v.extend_from_slice(s.as_bytes());
            v.push(0);
        }
        if strings.is_empty() {
            v.push(0);
        }
        v.push(0);
        v
    }

    /// A small but realistic table, wrapped in the Windows `RawSMBIOSData` header.
    pub(crate) fn sample_raw() -> Vec<u8> {
        let mut t = Vec::new();
        // Type 0: vendor=1, version=2, start segment, release date=3
        t.extend(structure(
            0,
            &[1, 2, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &["LENOVO", "N1CET90W (1.58 )", "06/26/2023"],
        ));
        let mut sys = vec![1, 2, 3, 4];
        sys.extend_from_slice(&[
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]);
        sys.extend_from_slice(&[6, 0, 0]);
        t.extend(structure(
            1,
            &sys,
            &["LENOVO", "20HRCTO1WW", "ThinkPad X1 Carbon 5th", "PF0ABCDE"],
        ));
        t.extend(structure(
            2,
            &[1, 2, 0, 3, 0],
            &["LENOVO", "20HRCTO1WW", "L1HF4AB0001"],
        ));
        // Type 3: manufacturer=1, type=0x8A (lock bit set, notebook 10), version=0, serial=0, asset=2
        t.extend(structure(3, &[1, 0x8A, 0, 0, 2], &["LENOVO", "ACME-0412"]));
        t.extend(structure(127, &[], &[]));
        let mut raw = vec![0, 3, 0, 0];
        raw.extend_from_slice(&(t.len() as u32).to_le_bytes());
        raw.extend(t);
        raw
    }

    #[test]
    fn parses_identity_structures() {
        let id = parse_raw_smbios_data(&sample_raw()).unwrap();
        assert_eq!(id.version, (3, 0));
        assert_eq!(id.bios_vendor.as_deref(), Some("LENOVO"));
        assert_eq!(id.bios_version.as_deref(), Some("N1CET90W (1.58 )"));
        assert_eq!(id.bios_date.as_deref(), Some("06/26/2023"));
        assert_eq!(id.product_name.as_deref(), Some("20HRCTO1WW"));
        assert_eq!(id.product_serial.as_deref(), Some("PF0ABCDE"));
        assert_eq!(
            id.product_uuid.as_deref(),
            Some("00112233-4455-6677-8899-aabbccddeeff")
        );
        assert_eq!(id.board_serial.as_deref(), Some("L1HF4AB0001"));
        assert_eq!(id.chassis_type, Some(10));
        assert_eq!(id.chassis_asset_tag.as_deref(), Some("ACME-0412"));
    }

    #[test]
    fn unset_uuid_is_absent() {
        assert_eq!(format_uuid(&[0xFF; 16], (3, 0)), None);
        assert_eq!(format_uuid(&[0; 16], (3, 0)), None);
        let pre26 = format_uuid(&(0u8..16).collect::<Vec<_>>(), (2, 4)).unwrap();
        assert_eq!(pre26, "00010203-0405-0607-0809-0a0b0c0d0e0f");
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        let raw = sample_raw();
        for cut in 0..raw.len() {
            let _ = parse_raw_smbios_data(&raw[..cut]);
        }
        let mut m = raw.clone();
        for i in 0..m.len() {
            for v in [0x00, 0x01, 0x7F, 0xFF] {
                let old = m[i];
                m[i] = v;
                let _ = parse_raw_smbios_data(&m);
                m[i] = old;
            }
        }
    }
}
