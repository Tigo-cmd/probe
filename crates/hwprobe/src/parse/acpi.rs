//! ACPI Windows Platform Binary Table (WPBT).
//!
//! WPBT is how firmware hands the operating system a binary to run at every
//! boot. Absolute (Computrace) persistence uses it to reinstall its agent
//! after a disk wipe; so do some OEM update utilities. Its presence means the
//! firmware, not the disk, carries software the buyer did not install.

use super::{require_len, ParseError};

const WPBT_FIXED_LEN: usize = 52;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wpbt {
    pub oem_id: String,
    pub oem_table_id: String,
    /// The command line the firmware asks Windows to run the binary with.
    pub command_line: Option<String>,
}

pub fn parse_wpbt(buf: &[u8]) -> Result<Wpbt, ParseError> {
    require_len(buf, WPBT_FIXED_LEN)?;
    if &buf[..4] != b"WPBT" {
        return Err(ParseError::BadHeader);
    }
    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    if len < WPBT_FIXED_LEN {
        return Err(ParseError::BadHeader);
    }
    require_len(buf, len)?;
    let table = &buf[..len];
    if table.iter().fold(0u8, |a, b| a.wrapping_add(*b)) != 0 {
        return Err(ParseError::BadChecksum);
    }

    let cmd_len = u16::from_le_bytes([table[50], table[51]]) as usize;
    let command_line = table
        .get(WPBT_FIXED_LEN..WPBT_FIXED_LEN.saturating_add(cmd_len))
        .map(utf16_le)
        .filter(|s| !s.is_empty());

    Ok(Wpbt {
        oem_id: ascii(&table[10..16]),
        oem_table_id: ascii(&table[16..24]),
        command_line,
    })
}

fn ascii(raw: &[u8]) -> String {
    raw.iter()
        .take_while(|&&b| b != 0)
        .filter(|b| b.is_ascii_graphic() || **b == b' ')
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

fn utf16_le(raw: &[u8]) -> String {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units).trim().to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> Vec<u8> {
        let cmd: Vec<u8> = "rpcnetp.exe /install"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut t = vec![0u8; WPBT_FIXED_LEN];
        t[..4].copy_from_slice(b"WPBT");
        t[10..16].copy_from_slice(b"LENOVO");
        t[16..24].copy_from_slice(b"TP-N1C  ");
        t[50..52].copy_from_slice(&(cmd.len() as u16).to_le_bytes());
        t.extend(cmd);
        let len = t.len() as u32;
        t[4..8].copy_from_slice(&len.to_le_bytes());
        let sum = t.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        t[9] = 0u8.wrapping_sub(sum);
        t
    }

    #[test]
    fn parses_wpbt() {
        let w = parse_wpbt(&sample()).unwrap();
        assert_eq!(w.oem_id, "LENOVO");
        assert_eq!(w.oem_table_id, "TP-N1C");
        assert_eq!(w.command_line.as_deref(), Some("rpcnetp.exe /install"));
    }

    #[test]
    fn rejects_other_tables_and_bad_checksums() {
        let mut t = sample();
        t[9] = t[9].wrapping_add(1);
        assert_eq!(parse_wpbt(&t), Err(ParseError::BadChecksum));
        t[..4].copy_from_slice(b"FACP");
        assert_eq!(parse_wpbt(&t), Err(ParseError::BadHeader));
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        let t = sample();
        for cut in 0..t.len() {
            let _ = parse_wpbt(&t[..cut]);
        }
        let mut m = t.clone();
        for i in 0..m.len() {
            for v in [0x00, 0x7F, 0xFF] {
                let old = m[i];
                m[i] = v;
                let _ = parse_wpbt(&m);
                m[i] = old;
            }
        }
    }
}
