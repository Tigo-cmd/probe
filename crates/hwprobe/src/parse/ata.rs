//! ATA SMART READ DATA (B0h / D0h) attribute table, 512 bytes.
//!
//! Only the table layout is standard. Attribute semantics above a small core
//! are vendor-defined, so the grader only acts on the five attributes that are
//! consistent across manufacturers (5, 187, 188, 197, 198).

use super::{require_len, ParseError};
use crate::model::{AtaAttribute, AtaHealth};

pub const SMART_DATA_LEN: usize = 512;
const TABLE_OFFSET: usize = 2;
const ENTRY_LEN: usize = 12;
const MAX_ENTRIES: usize = 30;

pub fn parse_smart_data(buf: &[u8]) -> Result<AtaHealth, ParseError> {
    require_len(buf, SMART_DATA_LEN)?;
    // Byte 511 is a checksum: the whole sector sums to zero mod 256. Some
    // drives leave it zero, so a mismatch is only an error when it is set.
    let sum = buf[..SMART_DATA_LEN]
        .iter()
        .fold(0u8, |acc, b| acc.wrapping_add(*b));
    if buf[511] != 0 && sum != 0 {
        return Err(ParseError::BadChecksum);
    }

    let attributes = buf[TABLE_OFFSET..TABLE_OFFSET + MAX_ENTRIES * ENTRY_LEN]
        .chunks_exact(ENTRY_LEN)
        .filter(|e| e[0] != 0)
        .map(|e| {
            let mut raw = [0u8; 8];
            raw[..6].copy_from_slice(&e[5..11]);
            AtaAttribute {
                id: e[0],
                current: e[3],
                worst: e[4],
                raw: u64::from_le_bytes(raw),
            }
        })
        .collect();
    Ok(AtaHealth { attributes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(buf: &mut [u8], slot: usize, id: u8, current: u8, worst: u8, raw: u64) {
        let o = TABLE_OFFSET + slot * ENTRY_LEN;
        buf[o] = id;
        buf[o + 3] = current;
        buf[o + 4] = worst;
        buf[o + 5..o + 11].copy_from_slice(&raw.to_le_bytes()[..6]);
    }

    #[test]
    fn parses_attribute_table_and_skips_empty_slots() {
        let mut data = [0u8; SMART_DATA_LEN];
        entry(&mut data, 0, 5, 100, 100, 8);
        entry(&mut data, 2, 197, 200, 200, 1);
        entry(&mut data, 3, 188, 100, 99, 0x0002_0001_0000_0003);

        let h = parse_smart_data(&data).unwrap();
        assert_eq!(h.attributes.len(), 3);
        assert_eq!(h.attribute(5).unwrap().count(), 8);
        assert_eq!(h.attribute(197).unwrap().count(), 1);
        // Upper bytes are truncated to 48 bits, and 188 counts the low 16 bits only.
        assert_eq!(h.attribute(188).unwrap().count(), 3);
        assert!(h.attribute(9).is_none());
    }

    #[test]
    fn validates_checksum_when_present() {
        let mut data = [0u8; SMART_DATA_LEN];
        entry(&mut data, 0, 5, 100, 100, 0);
        data[511] = 1;
        assert_eq!(parse_smart_data(&data), Err(ParseError::BadChecksum));

        let sum = data[..511].iter().fold(0u8, |a, b| a.wrapping_add(*b));
        data[511] = 0u8.wrapping_sub(sum);
        assert!(parse_smart_data(&data).is_ok());
    }
}
