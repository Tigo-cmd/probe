//! Output buffers of the Windows storage, battery and processor queries.
//!
//! The Windows backend issues the calls; decoding lives here so every layout
//! is tested on every platform, not only on Windows CI runners. Offsets follow
//! the structure definitions in `winioctl.h`, `batclass.h` and `winnt.h`.

use super::{require_len, ParseError};

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Decode a NUL-terminated UTF-16LE string, as the battery class returns them.
pub fn utf16_string(raw: &[u8]) -> Option<String> {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    let s = String::from_utf16_lossy(&units).trim().to_string();
    (!s.is_empty()).then_some(s)
}

// ----------------------------------------------------------------- storage

/// `STORAGE_BUS_TYPE` values that matter for grading.
pub const BUS_TYPE_ATA: u32 = 3;
pub const BUS_TYPE_USB: u32 = 7;
pub const BUS_TYPE_SATA: u32 = 11;
pub const BUS_TYPE_NVME: u32 = 17;

/// `STORAGE_DEVICE_DESCRIPTOR`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub removable: bool,
    pub bus_type: u32,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub revision: Option<String>,
    pub serial: Option<String>,
}

const DEVICE_DESCRIPTOR_FIXED: usize = 36;

pub fn parse_device_descriptor(buf: &[u8]) -> Result<DeviceDescriptor, ParseError> {
    require_len(buf, DEVICE_DESCRIPTOR_FIXED)?;
    let size = (u32_at(buf, 4) as usize).clamp(DEVICE_DESCRIPTOR_FIXED, buf.len());
    let buf = &buf[..size];
    let string = |field: usize| {
        let at = u32_at(buf, field) as usize;
        if at == 0 || at >= buf.len() {
            return None;
        }
        let raw = &buf[at..];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let s = String::from_utf8_lossy(&raw[..end]).trim().to_string();
        (!s.is_empty()).then_some(s)
    };
    Ok(DeviceDescriptor {
        removable: buf[10] != 0,
        bus_type: u32_at(buf, 28),
        vendor: string(12),
        product: string(16),
        revision: string(20),
        serial: string(24),
    })
}

/// `DEVICE_SEEK_PENALTY_DESCRIPTOR`: a seek penalty means a spinning disk.
pub fn parse_seek_penalty(buf: &[u8]) -> Result<bool, ParseError> {
    require_len(buf, 9)?;
    Ok(buf[8] != 0)
}

/// `DISK_GEOMETRY_EX.DiskSize`, in bytes.
pub fn parse_disk_geometry_size(buf: &[u8]) -> Result<u64, ParseError> {
    require_len(buf, 32)?;
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[24..32]);
    Ok(i64::from_le_bytes(b).max(0) as u64)
}

/// Where the protocol data starts in a `STORAGE_PROTOCOL_DATA_DESCRIPTOR`:
/// after `Version` and `Size`, at the offset `ProtocolSpecificData` names.
const PROTOCOL_SPECIFIC_AT: usize = 8;
const PROTOCOL_SPECIFIC_LEN: usize = 40;

/// The payload of a `STORAGE_PROTOCOL_DATA_DESCRIPTOR` returned for an NVMe
/// log page or identify query, checked to hold at least `want` bytes.
pub fn protocol_data(buf: &[u8], want: usize) -> Result<&[u8], ParseError> {
    require_len(buf, PROTOCOL_SPECIFIC_AT + PROTOCOL_SPECIFIC_LEN)?;
    let offset = u32_at(buf, PROTOCOL_SPECIFIC_AT + 16) as usize;
    let len = u32_at(buf, PROTOCOL_SPECIFIC_AT + 20) as usize;
    if offset < PROTOCOL_SPECIFIC_LEN || len < want {
        return Err(ParseError::BadHeader);
    }
    let start = PROTOCOL_SPECIFIC_AT + offset;
    buf.get(start..start.saturating_add(want))
        .ok_or(ParseError::TooShort {
            expected: start.saturating_add(want),
            got: buf.len(),
        })
}

/// `SENDCMDOUTPARAMS` from `SMART_RCV_DRIVE_DATA`: driver status, then the
/// 512-byte SMART data sector.
pub fn smart_out_params(buf: &[u8]) -> Result<&[u8], ParseError> {
    const DATA_AT: usize = 16;
    require_len(buf, DATA_AT + super::ata::SMART_DATA_LEN)?;
    if buf[4] != 0 {
        return Err(ParseError::BadHeader); // bDriverError
    }
    Ok(&buf[DATA_AT..DATA_AT + super::ata::SMART_DATA_LEN])
}

// ----------------------------------------------------------------- battery

/// `BATTERY_INFORMATION.Capabilities` bits.
pub const BATTERY_SYSTEM_BATTERY: u32 = 0x8000_0000;
pub const BATTERY_CAPACITY_RELATIVE: u32 = 0x4000_0000;
pub const BATTERY_UNKNOWN_CAPACITY: u32 = 0xFFFF_FFFF;

/// `BATTERY_INFORMATION`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatteryInformation {
    pub capabilities: u32,
    pub chemistry: Option<String>,
    /// In mWh unless [`BATTERY_CAPACITY_RELATIVE`] is set.
    pub designed_capacity: Option<u32>,
    pub full_charged_capacity: Option<u32>,
    pub cycle_count: u32,
}

pub fn parse_battery_information(buf: &[u8]) -> Result<BatteryInformation, ParseError> {
    require_len(buf, 36)?;
    let cap = |o| Some(u32_at(buf, o)).filter(|&v| v != BATTERY_UNKNOWN_CAPACITY && v != 0);
    let chemistry = buf[8..12]
        .iter()
        .take_while(|&&b| b != 0)
        .filter(|b| b.is_ascii_graphic())
        .map(|&b| b as char)
        .collect::<String>();
    Ok(BatteryInformation {
        capabilities: u32_at(buf, 0),
        chemistry: (!chemistry.is_empty()).then_some(chemistry),
        designed_capacity: cap(12),
        full_charged_capacity: cap(16),
        cycle_count: u32_at(buf, 32),
    })
}

/// `BATTERY_MANUFACTURE_DATE` as `YYYY-MM-DD`, if it is a plausible date.
pub fn parse_battery_date(buf: &[u8]) -> Option<String> {
    if buf.len() < 4 {
        return None;
    }
    let (day, month, year) = (buf[0], buf[1], u16_at(buf, 2));
    ((1..=31).contains(&day) && (1..=12).contains(&month) && (1990..=2100).contains(&year))
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

// --------------------------------------------------------------- processor

/// Count cores and logical processors in the `RelationProcessorCore` records
/// returned by `GetLogicalProcessorInformationEx`. `pointer_width` is the size
/// of `KAFFINITY` on the target (8 on 64-bit Windows).
pub fn count_processor_cores(buf: &[u8], pointer_width: usize) -> (u32, u32) {
    const RELATION_PROCESSOR_CORE: u32 = 0;
    const GROUP_COUNT_AT: usize = 30;
    const GROUP_MASK_AT: usize = 32;
    let group_affinity_len = if pointer_width == 8 { 16 } else { 12 };

    let (mut cores, mut logical) = (0u32, 0u32);
    let mut at = 0usize;
    while at + 8 <= buf.len() {
        let size = u32_at(buf, at + 4) as usize;
        if size < 8 || at.checked_add(size).map_or(true, |end| end > buf.len()) {
            break;
        }
        let rec = &buf[at..at + size];
        if u32_at(rec, 0) == RELATION_PROCESSOR_CORE && rec.len() >= GROUP_MASK_AT {
            cores += 1;
            let groups = u16_at(rec, GROUP_COUNT_AT) as usize;
            for g in 0..groups {
                let o = GROUP_MASK_AT + g * group_affinity_len;
                let Some(mask) = rec.get(o..o + pointer_width) else {
                    break;
                };
                logical += mask.iter().map(|b| b.count_ones()).sum::<u32>();
            }
        }
        at += size;
    }
    (cores, logical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(bus: u32, serial: &str) -> Vec<u8> {
        let mut b = vec![0u8; 40];
        b[10] = 0;
        b[28..32].copy_from_slice(&bus.to_le_bytes());
        let mut put = |field: usize, s: &str| {
            let at = b.len() as u32;
            b[field..field + 4].copy_from_slice(&at.to_le_bytes());
            b.extend_from_slice(s.as_bytes());
            b.push(0);
        };
        put(16, "Samsung SSD 970 EVO Plus 1TB");
        put(20, "2B2QEXM7");
        put(24, serial);
        let len = b.len() as u32;
        b[4..8].copy_from_slice(&len.to_le_bytes());
        b
    }

    #[test]
    fn parses_device_descriptor() {
        let d = parse_device_descriptor(&descriptor(BUS_TYPE_NVME, "  S4EWNX0R123456 ")).unwrap();
        assert_eq!(d.bus_type, BUS_TYPE_NVME);
        assert_eq!(d.vendor, None);
        assert_eq!(d.product.as_deref(), Some("Samsung SSD 970 EVO Plus 1TB"));
        assert_eq!(d.revision.as_deref(), Some("2B2QEXM7"));
        assert_eq!(d.serial.as_deref(), Some("S4EWNX0R123456"));
        assert!(!d.removable);
    }

    #[test]
    fn descriptor_offsets_past_the_end_are_ignored() {
        let mut b = descriptor(BUS_TYPE_SATA, "X");
        b[24..28].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        assert_eq!(parse_device_descriptor(&b).unwrap().serial, None);
    }

    #[test]
    fn extracts_protocol_payload() {
        let mut b = vec![0u8; 8 + 40 + 512];
        b[8 + 16..8 + 20].copy_from_slice(&40u32.to_le_bytes());
        b[8 + 20..8 + 24].copy_from_slice(&512u32.to_le_bytes());
        b[48] = 0xAB;
        let p = protocol_data(&b, 512).unwrap();
        assert_eq!(p.len(), 512);
        assert_eq!(p[0], 0xAB);
        b[8 + 16..8 + 20].copy_from_slice(&4000u32.to_le_bytes());
        assert!(protocol_data(&b, 512).is_err());
    }

    #[test]
    fn parses_battery_information() {
        let mut b = vec![0u8; 36];
        b[0..4].copy_from_slice(&BATTERY_SYSTEM_BATTERY.to_le_bytes());
        b[8..12].copy_from_slice(b"LION");
        b[12..16].copy_from_slice(&57_000u32.to_le_bytes());
        b[16..20].copy_from_slice(&BATTERY_UNKNOWN_CAPACITY.to_le_bytes());
        b[32..36].copy_from_slice(&412u32.to_le_bytes());
        let i = parse_battery_information(&b).unwrap();
        assert_eq!(i.chemistry.as_deref(), Some("LION"));
        assert_eq!(i.designed_capacity, Some(57_000));
        assert_eq!(i.full_charged_capacity, None);
        assert_eq!(i.cycle_count, 412);
        assert_eq!(
            parse_battery_date(&[14, 3, 0xE6, 0x07]).as_deref(),
            Some("2022-03-14")
        );
        assert_eq!(parse_battery_date(&[0, 0, 0, 0]), None);
    }

    #[test]
    fn counts_cores_and_threads() {
        // Two cores with SMT (two bits each), then one core without.
        let mut b = Vec::new();
        for mask in [0b11u64, 0b1100, 0b1_0000] {
            let mut r = vec![0u8; 48];
            r[4..8].copy_from_slice(&48u32.to_le_bytes());
            r[30..32].copy_from_slice(&1u16.to_le_bytes());
            r[32..40].copy_from_slice(&mask.to_le_bytes());
            b.extend(r);
        }
        assert_eq!(count_processor_cores(&b, 8), (3, 5));
        assert_eq!(count_processor_cores(&b[..50], 8), (1, 2));
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        let d = descriptor(BUS_TYPE_NVME, "S4EW");
        for cut in 0..d.len() {
            let _ = parse_device_descriptor(&d[..cut]);
            let _ = protocol_data(&d[..cut], 512);
            let _ = parse_battery_information(&d[..cut]);
            let _ = smart_out_params(&d[..cut]);
            let _ = count_processor_cores(&d[..cut], 8);
            let _ = parse_disk_geometry_size(&d[..cut]);
        }
        let junk = [0xFFu8; 64];
        let _ = parse_device_descriptor(&junk);
        let _ = protocol_data(&junk, 512);
        let _ = count_processor_cores(&junk, 8);
    }
}
