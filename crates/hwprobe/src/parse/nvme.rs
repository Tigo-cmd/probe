//! NVMe SMART / Health Information log page (Log Identifier 02h), 512 bytes.

use super::{le_u128, require_len, ParseError};
use crate::model::NvmeHealth;

pub const HEALTH_LOG_LEN: usize = 512;

pub fn parse_health_log(buf: &[u8]) -> Result<NvmeHealth, ParseError> {
    require_len(buf, HEALTH_LOG_LEN)?;
    Ok(NvmeHealth {
        critical_warning: buf[0],
        temperature_kelvin: u16::from_le_bytes([buf[1], buf[2]]),
        available_spare: buf[3],
        available_spare_threshold: buf[4],
        percentage_used: buf[5],
        data_units_read: le_u128(buf, 32),
        data_units_written: le_u128(buf, 48),
        power_cycles: le_u128(buf, 112),
        power_on_hours: le_u128(buf, 128),
        unsafe_shutdowns: le_u128(buf, 144),
        media_errors: le_u128(buf, 160),
        error_log_entries: le_u128(buf, 176),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_u128(buf: &mut [u8], offset: usize, v: u128) {
        buf[offset..offset + 16].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn parses_fields_at_spec_offsets() {
        let mut log = [0u8; HEALTH_LOG_LEN];
        log[0] = 0x04;
        log[1..3].copy_from_slice(&310u16.to_le_bytes());
        log[3] = 100;
        log[4] = 10;
        log[5] = 7;
        put_u128(&mut log, 48, 12_345_678);
        put_u128(&mut log, 128, 9_876);
        put_u128(&mut log, 144, 42);
        put_u128(&mut log, 160, 3);

        let h = parse_health_log(&log).unwrap();
        assert_eq!(h.critical_warning, 0x04);
        assert_eq!(h.temperature_kelvin, 310);
        assert_eq!(h.available_spare, 100);
        assert_eq!(h.available_spare_threshold, 10);
        assert_eq!(h.percentage_used, 7);
        assert_eq!(h.data_units_written, 12_345_678);
        assert_eq!(h.bytes_written(), 12_345_678 * 512_000);
        assert_eq!(h.power_on_hours, 9_876);
        assert_eq!(h.unsafe_shutdowns, 42);
        assert_eq!(h.media_errors, 3);
    }

    #[test]
    fn rejects_short_buffer() {
        assert_eq!(
            parse_health_log(&[0u8; 100]),
            Err(ParseError::TooShort {
                expected: 512,
                got: 100
            })
        );
    }
}
