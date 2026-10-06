//! EDID 1.x base block (128 bytes): panel identity and native resolution.

use super::{require_len, ParseError};
use crate::model::Display;

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
pub const BASE_BLOCK_LEN: usize = 128;

pub fn parse_edid(connector: &str, buf: &[u8]) -> Result<Display, ParseError> {
    require_len(buf, BASE_BLOCK_LEN)?;
    if buf[..8] != HEADER {
        return Err(ParseError::BadHeader);
    }
    if buf[..BASE_BLOCK_LEN]
        .iter()
        .fold(0u8, |a, b| a.wrapping_add(*b))
        != 0
    {
        return Err(ParseError::BadChecksum);
    }

    let mfg = u16::from_be_bytes([buf[8], buf[9]]);
    let manufacturer_id = [(mfg >> 10) & 0x1F, (mfg >> 5) & 0x1F, mfg & 0x1F]
        .iter()
        .map(|&c| (1..=26).contains(&c).then(|| (b'A' + c as u8 - 1) as char))
        .collect::<Option<String>>();

    let serial = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let week = buf[16];
    let year = buf[17];

    let mut display = Display {
        connector: connector.to_string(),
        manufacturer_id,
        product_code: Some(u16::from_le_bytes([buf[10], buf[11]])),
        serial: (serial != 0).then_some(serial),
        // Week 0xFF means byte 17 is a model year, not a manufacture year.
        manufacture_week: (1..=54).contains(&week).then_some(week),
        manufacture_year: (week != 0xFF && year != 0).then(|| 1990 + year as u16),
        ..Display::default()
    };

    for d in buf[54..126].chunks_exact(18) {
        let pixel_clock = u16::from_le_bytes([d[0], d[1]]);
        if pixel_clock != 0 {
            // The first detailed timing descriptor is the preferred (native) mode.
            if display.native_width.is_none() {
                display.native_width = Some(d[2] as u16 | ((d[4] as u16 & 0xF0) << 4));
                display.native_height = Some(d[5] as u16 | ((d[7] as u16 & 0xF0) << 4));
            }
        } else if d[3] == 0xFC {
            display.name = Some(descriptor_text(&d[5..18]));
        }
    }
    Ok(display)
}

fn descriptor_text(raw: &[u8]) -> String {
    raw.iter()
        .take_while(|&&b| b != 0x0A)
        .filter(|b| b.is_ascii_graphic() || **b == b' ')
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> [u8; 128] {
        let mut e = [0u8; 128];
        e[..8].copy_from_slice(&HEADER);
        // "LGD": L=12, G=7, D=4
        let mfg: u16 = (12 << 10) | (7 << 5) | 4;
        e[8..10].copy_from_slice(&mfg.to_be_bytes());
        e[10..12].copy_from_slice(&0x046Du16.to_le_bytes());
        e[16] = 12;
        e[17] = 23; // 2013
                    // Preferred timing: 1366x768
        e[54] = 0x1C;
        e[55] = 0x25;
        e[56] = (1366 & 0xFF) as u8;
        e[58] = ((1366 >> 8) as u8) << 4;
        e[59] = (768 & 0xFF) as u8;
        e[61] = ((768 >> 8) as u8) << 4;
        // Monitor name descriptor
        e[72 + 3] = 0xFC;
        e[72 + 5..72 + 5 + 13].copy_from_slice(b"LP140WH2\n    ");
        let sum = e[..127].iter().fold(0u8, |a, b| a.wrapping_add(*b));
        e[127] = 0u8.wrapping_sub(sum);
        e
    }

    #[test]
    fn parses_identity_and_native_mode() {
        let d = parse_edid("eDP-1", &sample()).unwrap();
        assert_eq!(d.manufacturer_id.as_deref(), Some("LGD"));
        assert_eq!(d.product_code, Some(0x046D));
        assert_eq!(d.manufacture_week, Some(12));
        assert_eq!(d.manufacture_year, Some(2013));
        assert_eq!((d.native_width, d.native_height), (Some(1366), Some(768)));
        assert_eq!(d.name.as_deref(), Some("LP140WH2"));
        assert_eq!(d.serial, None);
    }

    #[test]
    fn rejects_bad_header_and_checksum() {
        let mut e = sample();
        e[127] = e[127].wrapping_add(1);
        assert_eq!(parse_edid("x", &e), Err(ParseError::BadChecksum));
        e[0] = 1;
        assert_eq!(parse_edid("x", &e), Err(ParseError::BadHeader));
        assert!(parse_edid("x", &e[..20]).is_err());
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        let mut e = sample();
        for i in 0..128 {
            for v in [0x00, 0x7F, 0xFF] {
                e[i] = v;
                let _ = parse_edid("x", &e);
            }
        }
    }
}
