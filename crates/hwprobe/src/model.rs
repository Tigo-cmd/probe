//! The scan record: everything extracted from one machine at one point in time.
//!
//! Every value is wrapped in a [`Field`] carrying its epistemic class and the
//! interface it came from. A serial number read from SMBIOS is a *claim* (the
//! string is rewritable firmware); a media-error counter read from an NVMe
//! controller is a *measurement*. The UI and the grader must never blur the two.

use serde::{Deserialize, Serialize};

/// Bumped whenever the shape of [`Scan`] changes incompatibly.
pub const SCAN_SCHEMA_VERSION: u32 = 1;

/// The epistemic class of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Read from a sensor or controller (NVMe media errors, full-charge capacity).
    Measured,
    /// Read from rewritable firmware strings (SMBIOS model, serial, EDID name).
    Claimed,
    /// Derived from other values with an error band (TBW consumed, write rate).
    Inferred,
    /// Not determinable by software (hinge wear, swollen cell, liquid damage).
    NotAssessable,
}

/// A single extracted value with its provenance and source.
///
/// `value` is `None` when the platform did not report it. "Not reported" is
/// never the same as "healthy", and the grader treats it as missing evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field<T> {
    pub value: Option<T>,
    pub provenance: Provenance,
    /// The interface the value came from, e.g. `sysfs:/sys/class/dmi/id/product_name`.
    pub source: String,
    /// Why the value is missing or should be read with care.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl<T> Field<T> {
    pub fn new(value: Option<T>, provenance: Provenance, source: impl Into<String>) -> Self {
        Field {
            value,
            provenance,
            source: source.into(),
            note: None,
        }
    }

    pub fn measured(value: Option<T>, source: impl Into<String>) -> Self {
        Self::new(value, Provenance::Measured, source)
    }

    pub fn claimed(value: Option<T>, source: impl Into<String>) -> Self {
        Self::new(value, Provenance::Claimed, source)
    }

    pub fn missing(
        provenance: Provenance,
        source: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Field {
            value: None,
            provenance,
            source: source.into(),
            note: Some(note.into()),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Transform the value while keeping provenance, source and note.
    pub fn map<U>(&self, f: impl FnOnce(&T) -> U) -> Field<U> {
        Field {
            value: self.value.as_ref().map(f),
            provenance: self.provenance,
            source: self.source.clone(),
            note: self.note.clone(),
        }
    }
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Field {
            value: None,
            provenance: Provenance::Claimed,
            source: String::new(),
            note: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scan {
    pub schema_version: u32,
    pub tool_version: String,
    /// Operating system the backend ran on (`linux`, `windows`, `macos`).
    pub os: String,
    /// Unix seconds at scan start.
    pub started_at: u64,
    pub duration_ms: u64,
    /// Whether the scan ran with administrator/root privilege. Many health
    /// reads (SMART, serials) need it; an unprivileged scan is incomplete.
    pub elevated: bool,
    pub identity: Identity,
    pub cpu: Cpu,
    pub memory: Memory,
    pub batteries: Vec<Battery>,
    pub storage: Vec<Drive>,
    pub displays: Vec<Display>,
    pub network: Vec<NetworkInterface>,
    pub encumbrance: Vec<EncumbranceSignal>,
    /// Problems encountered while probing; a failed read is recorded, never hidden.
    pub probe_notes: Vec<ProbeNote>,
}

/// SMBIOS identity strings. All of these are claims: rewritable and spoofable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub vendor: Field<String>,
    pub model: Field<String>,
    pub version: Field<String>,
    pub serial: Field<String>,
    pub uuid: Field<String>,
    pub board_vendor: Field<String>,
    pub board_name: Field<String>,
    pub board_serial: Field<String>,
    pub bios_vendor: Field<String>,
    pub bios_version: Field<String>,
    pub bios_date: Field<String>,
    pub asset_tag: Field<String>,
    pub chassis_type: Field<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cpu {
    /// Brand string from CPUID. Comes from the processor, but CPUID can be forged.
    pub brand: Field<String>,
    pub logical_cores: Field<u32>,
    pub physical_cores: Field<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    /// Usable RAM as seen by the OS.
    pub total_bytes: Field<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityUnit {
    MilliwattHours,
    MilliampHours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    pub value: u64,
    pub unit: CapacityUnit,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    pub name: String,
    pub manufacturer: Field<String>,
    pub model: Field<String>,
    pub serial: Field<String>,
    pub technology: Field<String>,
    /// Design capacity as reported by the pack.
    pub design_capacity: Field<Capacity>,
    /// Full-charge capacity from the pack's own fuel gauge.
    pub full_charge_capacity: Field<Capacity>,
    pub cycle_count: Field<u32>,
    /// `YYYY-MM-DD` when the pack reports it.
    pub manufacture_date: Field<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Nvme,
    Sata,
    /// Behind a USB bridge: SMART is bridge-specific and often broken, and the
    /// drive is not the machine's internal drive. Never graded as internal.
    Usb,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drive {
    pub name: String,
    pub transport: Transport,
    pub rotational: Field<bool>,
    pub removable: bool,
    pub model: Field<String>,
    pub serial: Field<String>,
    pub firmware: Field<String>,
    /// Capacity reported by the drive controller. A reflashed controller can
    /// lie about this; only a write-then-verify pass measures it.
    pub capacity_bytes: Field<u64>,
    pub health: DriveHealth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriveHealth {
    Nvme(NvmeHealth),
    Ata(AtaHealth),
    Unavailable { reason: String },
}

/// NVMe SMART / Health Information log page (Log Identifier 02h).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvmeHealth {
    pub critical_warning: u8,
    pub temperature_kelvin: u16,
    pub available_spare: u8,
    pub available_spare_threshold: u8,
    /// Vendor estimate of life used; may legitimately exceed 100.
    pub percentage_used: u8,
    /// In units of 1000 × 512 bytes.
    #[serde(with = "u128_counter")]
    pub data_units_read: u128,
    #[serde(with = "u128_counter")]
    pub data_units_written: u128,
    #[serde(with = "u128_counter")]
    pub power_cycles: u128,
    #[serde(with = "u128_counter")]
    pub power_on_hours: u128,
    #[serde(with = "u128_counter")]
    pub unsafe_shutdowns: u128,
    #[serde(with = "u128_counter")]
    pub media_errors: u128,
    #[serde(with = "u128_counter")]
    pub error_log_entries: u128,
}

/// NVMe counters are 128-bit, but a scan is often read back through an
/// internally tagged enum ([`DriveHealth`]), where serde cannot buffer u128.
/// Counters are written as plain numbers when they fit in u64 (always, in
/// practice) and as decimal strings otherwise; both forms read back.
mod u128_counter {
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
        match u64::try_from(*v) {
            Ok(small) => s.serialize_u64(small),
            Err(_) => s.serialize_str(&v.to_string()),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
        struct Counter;
        impl Visitor<'_> for Counter {
            type Value = u128;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a non-negative integer or decimal string")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<u128, E> {
                Ok(v as u128)
            }
            fn visit_u128<E: de::Error>(self, v: u128) -> Result<u128, E> {
                Ok(v)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<u128, E> {
                u128::try_from(v).map_err(|_| E::custom("negative counter"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<u128, E> {
                v.parse()
                    .map_err(|_| E::custom("counter is not a decimal integer"))
            }
        }
        d.deserialize_any(Counter)
    }
}

impl NvmeHealth {
    pub fn bytes_written(&self) -> u128 {
        self.data_units_written.saturating_mul(512_000)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtaHealth {
    pub attributes: Vec<AtaAttribute>,
}

impl AtaHealth {
    pub fn attribute(&self, id: u8) -> Option<&AtaAttribute> {
        self.attributes.iter().find(|a| a.id == id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtaAttribute {
    pub id: u8,
    pub current: u8,
    pub worst: u8,
    /// The 48-bit raw value. Its meaning above the standard core is
    /// vendor-defined; see [`AtaAttribute::count`].
    pub raw: u64,
}

impl AtaAttribute {
    /// The raw value interpreted as an event count. Vendors pack extra data
    /// into the upper bytes of some attributes (Seagate packs three 16-bit
    /// counters into 188), so only the bits that hold the count are used.
    pub fn count(&self) -> u64 {
        match self.id {
            188 => self.raw & 0xFFFF,
            _ => self.raw & 0xFFFF_FFFF,
        }
    }
}

/// Panel identity from EDID. All claims: EDID is a rewritable EEPROM.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Display {
    pub connector: String,
    pub manufacturer_id: Option<String>,
    pub product_code: Option<u16>,
    pub serial: Option<u32>,
    pub name: Option<String>,
    pub manufacture_week: Option<u8>,
    pub manufacture_year: Option<u16>,
    pub native_width: Option<u16>,
    pub native_height: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    /// Burned-in or firmware MAC address. Software-changeable.
    pub mac: String,
}

/// Something that may stop the buyer from owning or using the machine:
/// management enrolment, firmware locks, ex-corporate markers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncumbranceSignal {
    pub id: String,
    pub present: bool,
    pub provenance: Provenance,
    pub source: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeNote {
    pub component: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved scan must load back, whatever its drives reported.
    #[test]
    fn nvme_scans_round_trip_through_json() {
        let health = NvmeHealth {
            data_units_written: 27_340_000,
            power_on_hours: 5_210,
            media_errors: u128::MAX,
            ..NvmeHealth::default()
        };
        let scan = Scan {
            storage: vec![Drive {
                name: "nvme0n1".into(),
                transport: Transport::Nvme,
                rotational: Field::default(),
                removable: false,
                model: Field::default(),
                serial: Field::default(),
                firmware: Field::default(),
                capacity_bytes: Field::default(),
                health: DriveHealth::Nvme(health),
            }],
            ..Scan::default()
        };
        let text = serde_json::to_string(&scan).unwrap();
        assert!(
            text.contains("\"power_on_hours\":5210"),
            "small counters stay numbers"
        );
        let back: Scan = serde_json::from_str(&text).unwrap();
        assert_eq!(back, scan);
    }
}
