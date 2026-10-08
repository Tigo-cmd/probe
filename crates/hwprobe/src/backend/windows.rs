//! Windows backend: the raw SMBIOS and ACPI tables, the storage, battery and
//! disk IOCTLs, SetupAPI for monitors and batteries, IP Helper for adapters,
//! and the registry for management state. No WMI, no COM, no driver.
//!
//! Every IOCTL issued here is a query. Opening `\\.\PhysicalDriveN` for SMART
//! needs read/write access only because `SMART_RCV_DRIVE_DATA` is defined with
//! those access bits; nothing in this module writes to a device.
//!
//! Inventory, battery and encumbrance reads work unelevated. Drive health and
//! NVMe identify need administrator.

use std::ffi::c_void;
use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::time::Instant;

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW,
    SetupDiOpenDevRegKey, DICS_FLAG_GLOBAL, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, DIREG_DEV,
    GUID_DEVCLASS_BATTERY, GUID_DEVCLASS_MONITOR, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND, ERROR_NO_MORE_ITEMS,
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows_sys::Win32::NetworkManagement::NetManagement::{
    NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
    KEY_READ, KEY_WOW64_64KEY, REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::{
    EnumSystemFirmwareTables, GetLogicalProcessorInformationEx, GetSystemFirmwareTable,
    GlobalMemoryStatusEx, RelationProcessorCore, MEMORYSTATUSEX,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::signals::{self, Enrolment, EntraJoin};
use super::{is_placeholder, wpbt_signal};
use crate::model::*;
use crate::parse::{self, winioctl};

pub struct WindowsBackend {
    _private: (),
}

impl WindowsBackend {
    pub fn system() -> Self {
        WindowsBackend { _private: () }
    }

    pub fn scan(&self) -> Scan {
        let start = Instant::now();
        let mut notes = Vec::new();
        let elevated = is_elevated();
        if !elevated {
            notes.push(note(
                "privilege",
                "not running as administrator: drive health and NVMe serials are unavailable",
            ));
        }

        let identity = identity(&mut notes);
        let mut scan = Scan {
            schema_version: SCAN_SCHEMA_VERSION,
            tool_version: crate::VERSION.to_string(),
            os: "windows".into(),
            started_at: crate::unix_now(),
            elevated,
            cpu: cpu(),
            memory: memory(),
            batteries: batteries(&mut notes),
            storage: storage(&mut notes),
            displays: displays(&mut notes),
            network: network(&mut notes),
            encumbrance: encumbrance(&identity, &mut notes),
            identity,
            ..Scan::default()
        };
        scan.probe_notes = notes;
        scan.duration_ms = start.elapsed().as_millis() as u64;
        scan
    }
}

// ---------------------------------------------------------------- identity

const RSMB: u32 = u32::from_be_bytes(*b"RSMB");
const ACPI: u32 = u32::from_be_bytes(*b"ACPI");

fn identity(notes: &mut Vec<ProbeNote>) -> Identity {
    let smbios = match firmware_table(RSMB, 0) {
        Ok(Some(raw)) => parse::smbios::parse_raw_smbios_data(&raw)
            .map_err(|e| format!("SMBIOS table rejected: {e}")),
        Ok(None) => Err("firmware publishes no SMBIOS table".into()),
        Err(e) => Err(format!("SMBIOS table unreadable: {e}")),
    };
    let s = match smbios {
        Ok(s) => s,
        Err(message) => {
            notes.push(note("identity", &message));
            let missing = || Field::missing(Provenance::Claimed, "smbios", message.clone());
            return Identity {
                vendor: missing(),
                model: missing(),
                version: missing(),
                serial: missing(),
                uuid: missing(),
                board_vendor: missing(),
                board_name: missing(),
                board_serial: missing(),
                bios_vendor: missing(),
                bios_version: missing(),
                bios_date: missing(),
                asset_tag: missing(),
                chassis_type: Field::missing(Provenance::Claimed, "smbios", message.clone()),
            };
        }
    };
    let f = |v: Option<String>, name: &str| {
        let src = format!("smbios:{name}");
        match v {
            Some(v) if is_placeholder(&v) => Field::missing(
                Provenance::Claimed,
                src,
                format!("OEM placeholder {:?}", v.trim()),
            ),
            v => Field::claimed(v, src),
        }
    };
    Identity {
        vendor: f(s.sys_vendor, "type1.manufacturer"),
        model: f(s.product_name, "type1.product_name"),
        version: f(s.product_version, "type1.version"),
        serial: f(s.product_serial, "type1.serial_number"),
        uuid: f(s.product_uuid, "type1.uuid"),
        board_vendor: f(s.board_vendor, "type2.manufacturer"),
        board_name: f(s.board_name, "type2.product"),
        board_serial: f(s.board_serial, "type2.serial_number"),
        bios_vendor: f(s.bios_vendor, "type0.vendor"),
        bios_version: f(s.bios_version, "type0.version"),
        bios_date: f(s.bios_date, "type0.release_date"),
        asset_tag: f(s.chassis_asset_tag, "type3.asset_tag"),
        chassis_type: Field::claimed(s.chassis_type, "smbios:type3.type"),
    }
}

/// `Ok(None)` when the firmware does not publish the table.
fn firmware_table(provider: u32, id: u32) -> io::Result<Option<Vec<u8>>> {
    // SAFETY: a null buffer of size 0 asks for the required size.
    let need = unsafe { GetSystemFirmwareTable(provider, id, null_mut(), 0) };
    if need == 0 {
        let e = io::Error::last_os_error();
        return match e.raw_os_error() {
            Some(c) if c as u32 == ERROR_NOT_FOUND => Ok(None),
            _ => Err(e),
        };
    }
    let mut buf = vec![0u8; need as usize];
    // SAFETY: buf is `need` bytes long.
    let got = unsafe { GetSystemFirmwareTable(provider, id, buf.as_mut_ptr(), need) };
    if got == 0 || got > need {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(got as usize);
    Ok(Some(buf))
}

/// The WPBT, found by enumerating the ACPI tables rather than by asking for
/// it directly, so an empty enumeration is an error and never "no table".
fn acpi_wpbt() -> Result<Option<Vec<u8>>, String> {
    // SAFETY: a null buffer of size 0 asks for the required size.
    let need = unsafe { EnumSystemFirmwareTables(ACPI, null_mut(), 0) };
    if need == 0 {
        return Err(format!(
            "ACPI tables not enumerable: {}",
            io::Error::last_os_error()
        ));
    }
    let mut ids = vec![0u8; need as usize];
    // SAFETY: ids is `need` bytes long.
    let got = unsafe { EnumSystemFirmwareTables(ACPI, ids.as_mut_ptr(), need) };
    if got == 0 || got > need {
        return Err(format!(
            "ACPI tables not enumerable: {}",
            io::Error::last_os_error()
        ));
    }
    ids.truncate(got as usize);
    let Some(id) = ids
        .chunks_exact(4)
        .find(|c| *c == b"WPBT" || *c == b"TBPW")
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
    else {
        return Ok(None);
    };
    match firmware_table(ACPI, id) {
        Ok(Some(t)) => Ok(Some(t)),
        Ok(None) => Err("WPBT listed but not returned".into()),
        Err(e) => Err(format!("WPBT unreadable: {e}")),
    }
}

// ------------------------------------------------------------- cpu, memory

fn cpu() -> Cpu {
    const KEY: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";
    let brand_src = format!("cpuid via registry:HKLM\\{KEY}\\ProcessorNameString");
    let brand = open_key(KEY)
        .ok()
        .flatten()
        .and_then(|k| k.string("ProcessorNameString"));

    let src = "GetLogicalProcessorInformationEx(RelationProcessorCore)";
    let (cores, logical) = processor_cores().unwrap_or((0, 0));
    Cpu {
        brand: Field::measured(brand, brand_src),
        logical_cores: Field::measured((logical > 0).then_some(logical), src),
        physical_cores: Field::measured((cores > 0).then_some(cores), src),
    }
}

fn processor_cores() -> Option<(u32, u32)> {
    let mut len = 0u32;
    // SAFETY: a null buffer asks for the required length.
    unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, null_mut(), &mut len) };
    if len == 0 {
        return None;
    }
    // u64 storage keeps the records 8-byte aligned, as the API expects.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: buf holds at least `len` bytes.
    let ok = unsafe {
        GetLogicalProcessorInformationEx(RelationProcessorCore, buf.as_mut_ptr().cast(), &mut len)
    };
    if ok == 0 {
        return None;
    }
    // SAFETY: reinterpreting initialised u64 storage as bytes.
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len as usize) };
    Some(winioctl::count_processor_cores(bytes, size_of::<usize>()))
}

fn memory() -> Memory {
    // SAFETY: MEMORYSTATUSEX is plain data; dwLength is set before the call.
    let mut m: MEMORYSTATUSEX = unsafe { zeroed() };
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut m) } != 0;
    Memory {
        total_bytes: Field::measured(ok.then_some(m.ullTotalPhys), "GlobalMemoryStatusEx")
            .with_note("usable RAM after firmware and GPU reservations, not installed RAM"),
    }
}

// ----------------------------------------------------------------- battery

const FILE_DEVICE_BATTERY: u32 = 0x29;
const IOCTL_BATTERY_QUERY_TAG: u32 = ctl_code(FILE_DEVICE_BATTERY, 0x10, FILE_READ_ACCESS);
const IOCTL_BATTERY_QUERY_INFORMATION: u32 = ctl_code(FILE_DEVICE_BATTERY, 0x11, FILE_READ_ACCESS);

/// `BATTERY_QUERY_INFORMATION_LEVEL`.
#[derive(Clone, Copy)]
enum BatteryLevel {
    Information = 0,
    DeviceName = 4,
    ManufactureDate = 5,
    ManufactureName = 6,
    SerialNumber = 8,
}

fn batteries(notes: &mut Vec<ProbeNote>) -> Vec<Battery> {
    let paths = match interface_paths(&GUID_DEVCLASS_BATTERY) {
        Ok(p) => p,
        Err(e) => {
            notes.push(note(
                "battery",
                &format!("battery devices not enumerable: {e}"),
            ));
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for path in paths {
        match battery(&path, out.len()) {
            Ok(Some(b)) => out.push(b),
            Ok(None) => {}
            Err(e) => notes.push(note("battery", &format!("{path}: {e}"))),
        }
    }
    out
}

/// `Ok(None)` for an empty bay or a battery that does not power the system (a UPS).
fn battery(path: &str, index: usize) -> io::Result<Option<Battery>> {
    let h = open(path, GENERIC_READ)?;
    let tag_bytes = ioctl(&h, IOCTL_BATTERY_QUERY_TAG, &0u32.to_le_bytes(), 4)?;
    let tag = match tag_bytes.get(..4) {
        Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        None => return Ok(None),
    };
    if tag == 0 {
        return Ok(None);
    }
    let query = |level: BatteryLevel, out_len: usize| {
        let mut q = Vec::with_capacity(12);
        q.extend_from_slice(&tag.to_le_bytes());
        q.extend_from_slice(&(level as u32).to_le_bytes());
        q.extend_from_slice(&0i32.to_le_bytes());
        ioctl(&h, IOCTL_BATTERY_QUERY_INFORMATION, &q, out_len)
    };

    let info_src = "IOCTL_BATTERY_QUERY_INFORMATION(BatteryInformation)";
    let info = winioctl::parse_battery_information(&query(BatteryLevel::Information, 64)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    if info.capabilities & winioctl::BATTERY_SYSTEM_BATTERY == 0 {
        return Ok(None);
    }
    let text = |level: BatteryLevel, name: &str| {
        let src = format!("IOCTL_BATTERY_QUERY_INFORMATION({name})");
        let v = query(level, 512)
            .ok()
            .and_then(|b| winioctl::utf16_string(&b))
            .filter(|v| !is_placeholder(v));
        Field::claimed(v, src)
    };

    let relative = info.capabilities & winioctl::BATTERY_CAPACITY_RELATIVE != 0;
    let capacity = |v: Option<u32>, provenance| {
        if relative {
            return Field::missing(
                provenance,
                info_src,
                "pack reports relative capacity, not mWh",
            );
        }
        Field::new(
            v.map(|v| Capacity {
                value: v as u64,
                unit: CapacityUnit::MilliwattHours,
            }),
            provenance,
            info_src,
        )
    };
    let cycle_count = match info.cycle_count {
        0 => Field::missing(
            Provenance::Measured,
            info_src,
            "reported as 0, treated as not reported",
        ),
        n => Field::measured(Some(n), info_src),
    };
    let date = query(BatteryLevel::ManufactureDate, 4)
        .ok()
        .and_then(|b| winioctl::parse_battery_date(&b));

    Ok(Some(Battery {
        name: format!("BAT{index}"),
        manufacturer: text(BatteryLevel::ManufactureName, "BatteryManufactureName"),
        model: text(BatteryLevel::DeviceName, "BatteryDeviceName"),
        serial: text(BatteryLevel::SerialNumber, "BatterySerialNumber"),
        technology: Field::claimed(info.chemistry, info_src),
        // The design figure is a rewritable value in the pack's EEPROM.
        design_capacity: capacity(info.designed_capacity, Provenance::Claimed),
        full_charge_capacity: capacity(info.full_charged_capacity, Provenance::Measured),
        cycle_count,
        manufacture_date: Field::claimed(
            date,
            "IOCTL_BATTERY_QUERY_INFORMATION(BatteryManufactureDate)",
        ),
    }))
}

// ----------------------------------------------------------------- storage

const FILE_DEVICE_DISK: u32 = 0x07;
const IOCTL_STORAGE_BASE: u32 = 0x2D;
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = ctl_code(IOCTL_STORAGE_BASE, 0x500, FILE_ANY_ACCESS);
const IOCTL_DISK_GET_DRIVE_GEOMETRY_EX: u32 = ctl_code(FILE_DEVICE_DISK, 0x28, FILE_ANY_ACCESS);
const SMART_RCV_DRIVE_DATA: u32 =
    ctl_code(FILE_DEVICE_DISK, 0x22, FILE_READ_ACCESS | FILE_WRITE_ACCESS);

/// `STORAGE_PROPERTY_ID` values.
const STORAGE_DEVICE_PROPERTY: u32 = 0;
const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: u32 = 7;
const STORAGE_ADAPTER_PROTOCOL_SPECIFIC_PROPERTY: u32 = 49;
const STORAGE_DEVICE_PROTOCOL_SPECIFIC_PROPERTY: u32 = 50;

/// `STORAGE_PROTOCOL_NVME_DATA_TYPE` values.
const NVME_DATA_TYPE_IDENTIFY: u32 = 1;
const NVME_DATA_TYPE_LOG_PAGE: u32 = 2;
const NVME_IDENTIFY_CNS_CONTROLLER: u32 = 1;
const NVME_LOG_PAGE_HEALTH_INFO: u32 = 2;

const MAX_PHYSICAL_DRIVES: u32 = 32;

fn storage(notes: &mut Vec<ProbeNote>) -> Vec<Drive> {
    let mut out = Vec::new();
    for n in 0..MAX_PHYSICAL_DRIVES {
        let name = format!("PhysicalDrive{n}");
        let path = format!(r"\\.\{name}");
        // Access 0 is enough for property and geometry queries, unelevated.
        let h = match open(&path, 0) {
            Ok(h) => h,
            Err(e) if e.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => continue,
            Err(e) => {
                notes.push(note(&format!("storage/{name}"), &e.to_string()));
                continue;
            }
        };
        match drive(&name, &path, &h) {
            Ok(Some(d)) => {
                if let DriveHealth::Unavailable { reason } = &d.health {
                    notes.push(note(&format!("storage/{name}"), reason));
                }
                out.push(d);
            }
            Ok(None) => {}
            Err(e) => notes.push(note(&format!("storage/{name}"), &e)),
        }
    }
    out
}

/// `Ok(None)` for an empty slot, such as a card reader with no card.
fn drive(name: &str, path: &str, h: &Handle) -> Result<Option<Drive>, String> {
    let desc_src = "IOCTL_STORAGE_QUERY_PROPERTY(StorageDeviceProperty)";
    let desc = ioctl(
        h,
        IOCTL_STORAGE_QUERY_PROPERTY,
        &property_query(STORAGE_DEVICE_PROPERTY),
        1024,
    )
    .map_err(|e| format!("storage device descriptor unreadable: {e}"))
    .and_then(|b| {
        winioctl::parse_device_descriptor(&b)
            .map_err(|e| format!("storage device descriptor rejected: {e}"))
    })?;
    let transport = match desc.bus_type {
        winioctl::BUS_TYPE_USB => Transport::Usb,
        winioctl::BUS_TYPE_NVME => Transport::Nvme,
        winioctl::BUS_TYPE_SATA | winioctl::BUS_TYPE_ATA => Transport::Sata,
        _ => Transport::Other,
    };

    let size_src = "IOCTL_DISK_GET_DRIVE_GEOMETRY_EX";
    let size = ioctl(h, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, &[], 256)
        .ok()
        .and_then(|b| winioctl::parse_disk_geometry_size(&b).ok());
    if size == Some(0) {
        return Ok(None); // empty card reader slot
    }
    let rotational = ioctl(
        h,
        IOCTL_STORAGE_QUERY_PROPERTY,
        &property_query(STORAGE_DEVICE_SEEK_PENALTY_PROPERTY),
        16,
    )
    .ok()
    .and_then(|b| winioctl::parse_seek_penalty(&b).ok());

    let clean = |v: Option<String>| v.filter(|v| !is_placeholder(v));
    let mut model = Field::claimed(clean(desc.product), desc_src);
    let mut serial = Field::claimed(clean(desc.serial), desc_src);
    let mut firmware = Field::claimed(clean(desc.revision), desc_src);

    let health = match transport {
        Transport::Usb => DriveHealth::Unavailable {
            reason: "drive behind USB bridge: health data unverified".into(),
        },
        Transport::Other => DriveHealth::Unavailable {
            reason: "virtual or unsupported transport".into(),
        },
        Transport::Nvme => match open(path, GENERIC_READ) {
            Err(e) => unavailable(path, e),
            Ok(rh) => {
                // The descriptor's serial for NVMe is often the namespace EUI-64;
                // Identify Controller gives the serial Linux reports.
                let id_src = "IOCTL_STORAGE_QUERY_PROPERTY(NVMe Identify Controller)";
                if let Some(id) = nvme_query(
                    &rh,
                    STORAGE_ADAPTER_PROTOCOL_SPECIFIC_PROPERTY,
                    NVME_DATA_TYPE_IDENTIFY,
                    NVME_IDENTIFY_CNS_CONTROLLER,
                    parse::nvme::IDENTIFY_LEN,
                )
                .ok()
                .and_then(|b| parse::nvme::parse_identify_controller(&b).ok())
                {
                    if id.serial.is_some() {
                        serial = Field::claimed(clean(id.serial), id_src);
                    }
                    if id.model.is_some() {
                        model = Field::claimed(clean(id.model), id_src);
                    }
                    if id.firmware.is_some() {
                        firmware = Field::claimed(clean(id.firmware), id_src);
                    }
                }
                match nvme_query(
                    &rh,
                    STORAGE_DEVICE_PROTOCOL_SPECIFIC_PROPERTY,
                    NVME_DATA_TYPE_LOG_PAGE,
                    NVME_LOG_PAGE_HEALTH_INFO,
                    parse::nvme::HEALTH_LOG_LEN,
                ) {
                    Err(e) => unavailable(path, e),
                    Ok(log) => match parse::nvme::parse_health_log(&log) {
                        Ok(h) => DriveHealth::Nvme(h),
                        Err(e) => DriveHealth::Unavailable {
                            reason: format!("NVMe health log rejected: {e}"),
                        },
                    },
                }
            }
        },
        Transport::Sata => match open(path, GENERIC_READ | GENERIC_WRITE) {
            Err(e) => unavailable(path, e),
            Ok(rh) => match ioctl(
                &rh,
                SMART_RCV_DRIVE_DATA,
                &smart_read_data_params(),
                16 + 512,
            ) {
                Err(e) => unavailable(path, e),
                Ok(out) => {
                    match winioctl::smart_out_params(&out).and_then(parse::ata::parse_smart_data) {
                        Ok(h) => DriveHealth::Ata(h),
                        Err(e) => DriveHealth::Unavailable {
                            reason: format!("SMART data rejected: {e}"),
                        },
                    }
                }
            },
        },
    };

    Ok(Some(Drive {
        name: name.to_string(),
        transport,
        rotational: Field::measured(
            rotational,
            "IOCTL_STORAGE_QUERY_PROPERTY(StorageDeviceSeekPenaltyProperty)",
        ),
        removable: desc.removable,
        model,
        serial,
        firmware,
        capacity_bytes: Field::claimed(size, size_src)
            .with_note("reported by the drive controller; not verified by write-then-read"),
        health,
    }))
}

/// `STORAGE_PROPERTY_QUERY` for a standard query of `property`.
fn property_query(property: u32) -> Vec<u8> {
    let mut q = vec![0u8; 12];
    q[..4].copy_from_slice(&property.to_le_bytes());
    q
}

/// A `STORAGE_PROPERTY_QUERY` carrying `STORAGE_PROTOCOL_SPECIFIC_DATA` for
/// an NVMe identify or log-page read; returns the payload.
fn nvme_query(
    h: &Handle,
    property: u32,
    data_type: u32,
    value: u32,
    len: usize,
) -> io::Result<Vec<u8>> {
    const PROTOCOL_TYPE_NVME: u32 = 3;
    const SPECIFIC_LEN: u32 = 40;
    let mut q = vec![0u8; 8 + SPECIFIC_LEN as usize + len];
    let mut put = |at: usize, v: u32| q[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(0, property);
    put(4, 0); // PropertyStandardQuery
    put(8, PROTOCOL_TYPE_NVME);
    put(12, data_type);
    put(16, value);
    put(20, 0); // ProtocolDataRequestSubValue
    put(24, SPECIFIC_LEN); // ProtocolDataOffset, from the start of the specific data
    put(28, len as u32);
    let out = ioctl(h, IOCTL_STORAGE_QUERY_PROPERTY, &q, q.len())?;
    winioctl::protocol_data(&out, len)
        .map(<[u8]>::to_vec)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// `SENDCMDINPARAMS` (packed, 33 bytes) for SMART READ DATA (B0h / D0h).
fn smart_read_data_params() -> Vec<u8> {
    let mut p = vec![0u8; 33];
    p[..4].copy_from_slice(&512u32.to_le_bytes()); // cBufferSize
                                                   // IDEREGS: features, sector count, sector number, cyl low, cyl high, drive/head, command
    p[4..11].copy_from_slice(&[0xD0, 1, 1, 0x4F, 0xC2, 0xA0, 0xB0]);
    p
}

fn unavailable(path: &str, e: io::Error) -> DriveHealth {
    let reason = if e.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
        format!("{path}: requires administrator")
    } else {
        format!("{path}: {e}")
    };
    DriveHealth::Unavailable { reason }
}

// ---------------------------------------------------------------- displays

fn displays(notes: &mut Vec<ProbeNote>) -> Vec<Display> {
    let mut out = Vec::new();
    let Some(set) = DevInfo::new(&GUID_DEVCLASS_MONITOR, DIGCF_PRESENT) else {
        notes.push(note("display", "monitor devices not enumerable"));
        return out;
    };
    for i in 0.. {
        // SAFETY: SP_DEVINFO_DATA is plain data; cbSize is set before use.
        let mut info: SP_DEVINFO_DATA = unsafe { zeroed() };
        info.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        // SAFETY: set is a live device information set.
        if unsafe { SetupDiEnumDeviceInfo(set.0, i, &mut info) } == 0 {
            break;
        }
        let connector = instance_id(&set, &info)
            .and_then(|id| id.split('\\').nth(1).map(str::to_string))
            .unwrap_or_else(|| format!("monitor{i}"));
        // SAFETY: set and info are live; the returned key is closed by Key.
        let key =
            unsafe { SetupDiOpenDevRegKey(set.0, &info, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ) };
        if std::ptr::eq(key, INVALID_HANDLE_VALUE) || key.is_null() {
            continue;
        }
        let Some(edid) = Key(key).binary("EDID") else {
            continue;
        };
        match parse::edid::parse_edid(&connector, &edid) {
            Ok(d) => out.push(d),
            Err(e) => notes.push(note(
                &format!("display/{connector}"),
                &format!("EDID rejected: {e}"),
            )),
        }
    }
    out
}

fn instance_id(set: &DevInfo, info: &SP_DEVINFO_DATA) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut need = 0u32;
    // SAFETY: buf holds the stated number of UTF-16 units.
    let ok = unsafe {
        SetupDiGetDeviceInstanceIdW(set.0, info, buf.as_mut_ptr(), buf.len() as u32, &mut need)
    };
    (ok != 0).then(|| from_wide(&buf))
}

// ----------------------------------------------------------------- network

fn network(notes: &mut Vec<ProbeNote>) -> Vec<NetworkInterface> {
    const HARDWARE_INTERFACE: u8 = 1 << 0;
    const FILTER_INTERFACE: u8 = 1 << 1;
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    // SAFETY: GetIfTable2 allocates the table; it is released with FreeMibTable.
    let rc = unsafe { GetIfTable2(&mut table) };
    if rc != 0 || table.is_null() {
        notes.push(note("network", &format!("GetIfTable2 failed: error {rc}")));
        return Vec::new();
    }
    let mut out: Vec<NetworkInterface> = Vec::new();
    // SAFETY: the table holds NumEntries rows laid out contiguously.
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    for row in rows {
        let flags = row.InterfaceAndOperStatusFlags._bitfield;
        // Only physical adapters: skips loopback, tunnels, VPNs and the
        // per-adapter filter-driver interfaces Windows lists alongside them.
        if flags & HARDWARE_INTERFACE == 0 || flags & FILTER_INTERFACE != 0 {
            continue;
        }
        if row.PhysicalAddressLength != 6 {
            continue;
        }
        let burned_in = &row.PermanentPhysicalAddress[..6];
        let addr = if burned_in.iter().any(|&b| b != 0) {
            burned_in
        } else {
            &row.PhysicalAddress[..6]
        };
        if addr.iter().all(|&b| b == 0) {
            continue;
        }
        let mac = addr
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":");
        if out.iter().any(|n| n.mac == mac) {
            continue;
        }
        out.push(NetworkInterface {
            name: from_wide(&row.Alias),
            mac,
        });
    }
    // SAFETY: table came from GetIfTable2 and is freed once.
    unsafe { FreeMibTable(table as *const c_void) };
    out
}

// ------------------------------------------------------------- encumbrance

const ENROLLMENTS: &str = r"SOFTWARE\Microsoft\Enrollments";
const JOIN_INFO: &str = r"SYSTEM\CurrentControlSet\Control\CloudDomainJoin\JoinInfo";
const TENANT_INFO: &str = r"SYSTEM\CurrentControlSet\Control\CloudDomainJoin\TenantInfo";
const AUTOPILOT: &str = r"SOFTWARE\Microsoft\Provisioning\Diagnostics\Autopilot";
const SERVICES: &str = r"SYSTEM\CurrentControlSet\Services";

/// Service and file names of the Absolute (Computrace / LoJack) Windows agent.
const ABSOLUTE_SERVICES: &[&str] = &["rpcnet", "rpcnetp"];
const ABSOLUTE_FILES: &[&str] = &["rpcnet.exe", "rpcnetp.exe", "rpcnetp.dll"];

fn encumbrance(identity: &Identity, notes: &mut Vec<ProbeNote>) -> Vec<EncumbranceSignal> {
    let mut out = vec![signals::asset_tag(identity)];
    out.extend(wpbt_signal(
        acpi_wpbt(),
        "GetSystemFirmwareTable(ACPI, WPBT)",
        notes,
    ));

    let mut checked = |id: &str, r: Result<EncumbranceSignal, String>| match r {
        Ok(s) => out.push(s),
        Err(e) => notes.push(note(&format!("encumbrance/{id}"), &e)),
    };
    checked(signals::MDM_ENROLMENT, mdm_enrolment());
    checked(signals::ENTRA_JOIN, entra_join());
    checked(signals::DOMAIN_JOIN, domain_join());
    checked(signals::AUTOPILOT_PROFILE, autopilot_profile());
    checked(signals::ABSOLUTE_AGENT, absolute_agent());
    out
}

fn reg_source(path: &str) -> String {
    format!("registry:HKLM\\{path}")
}

fn mdm_enrolment() -> Result<EncumbranceSignal, String> {
    let mut found = Vec::new();
    if let Some(root) = open_key(ENROLLMENTS)? {
        for sub in root.subkeys().into_iter().filter(|s| is_guid(s)) {
            let Some(k) = open_key(&format!("{ENROLLMENTS}\\{sub}"))? else {
                continue;
            };
            let Some(provider) = k.string("ProviderID").filter(|p| !p.is_empty()) else {
                continue;
            };
            let upn = k.string("UPN").filter(|u| !u.is_empty());
            let discovery = k
                .string("DiscoveryServiceFullURL")
                .filter(|u| !u.is_empty());
            // Windows keeps provider stubs for built-in channels; a real
            // enrolment names a user or a discovery service.
            if upn.is_some() || discovery.is_some() {
                found.push(Enrolment { provider, upn });
            }
        }
    }
    Ok(signals::mdm_enrolment(&found, &reg_source(ENROLLMENTS)))
}

fn entra_join() -> Result<EncumbranceSignal, String> {
    let mut joins = Vec::new();
    if let Some(root) = open_key(JOIN_INFO)? {
        for sub in root.subkeys() {
            let Some(k) = open_key(&format!("{JOIN_INFO}\\{sub}"))? else {
                continue;
            };
            let Some(tenant_id) = k.string("TenantId").filter(|t| !t.is_empty()) else {
                continue;
            };
            let tenant_name = open_key(&format!("{TENANT_INFO}\\{tenant_id}"))
                .ok()
                .flatten()
                .and_then(|t| t.string("DisplayName"))
                .filter(|n| !n.is_empty());
            joins.push(EntraJoin {
                tenant_name,
                user_email: k.string("UserEmail"),
                tenant_id,
            });
        }
    }
    Ok(signals::entra_join(&joins, &reg_source(JOIN_INFO)))
}

fn domain_join() -> Result<EncumbranceSignal, String> {
    let mut name: *mut u16 = null_mut();
    let mut status = 0;
    // SAFETY: on success Windows allocates `name`, released with NetApiBufferFree.
    let rc = unsafe { NetGetJoinInformation(null(), &mut name, &mut status) };
    if rc != 0 {
        return Err(format!("NetGetJoinInformation failed: error {rc}"));
    }
    let domain = (!name.is_null() && status == NetSetupDomainName).then(|| {
        // SAFETY: name is a NUL-terminated string allocated by NetGetJoinInformation.
        unsafe { from_wide_ptr(name) }
    });
    if !name.is_null() {
        // SAFETY: name was allocated by the network management API.
        unsafe { NetApiBufferFree(name as *const c_void) };
    }
    Ok(signals::domain_join(
        domain.as_deref().filter(|d| !d.is_empty()),
        "NetGetJoinInformation",
    ))
}

fn autopilot_profile() -> Result<EncumbranceSignal, String> {
    let tenant = open_key(AUTOPILOT)?.and_then(|k| {
        k.string("CloudAssignedTenantDomain")
            .filter(|d| !d.is_empty())
            .or_else(|| k.string("CloudAssignedTenantId").filter(|d| !d.is_empty()))
    });
    Ok(signals::autopilot_profile(
        tenant.as_deref(),
        &reg_source(AUTOPILOT),
    ))
}

fn absolute_agent() -> Result<EncumbranceSignal, String> {
    let mut found = Vec::new();
    for svc in ABSOLUTE_SERVICES {
        if open_key(&format!("{SERVICES}\\{svc}"))?.is_some() {
            found.push(format!("service {svc}"));
        }
    }
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    for dir in ["System32", "SysWOW64"] {
        for file in ABSOLUTE_FILES {
            let p = std::path::Path::new(&root).join(dir).join(file);
            if p.exists() {
                found.push(p.display().to_string());
            }
        }
    }
    Ok(signals::absolute_agent(
        &found,
        &format!("{} and %SystemRoot%\\System32", reg_source(SERVICES)),
    ))
}

fn is_guid(s: &str) -> bool {
    let s = s.trim_start_matches('{').trim_end_matches('}');
    s.len() == 36
        && s.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

// ----------------------------------------------------------------- helpers

const FILE_ANY_ACCESS: u32 = 0;
const FILE_READ_ACCESS: u32 = 1;
const FILE_WRITE_ACCESS: u32 = 2;

/// `CTL_CODE` with `METHOD_BUFFERED`.
const fn ctl_code(device: u32, function: u32, access: u32) -> u32 {
    (device << 16) | (access << 14) | (function << 2)
}

fn note(component: &str, message: &str) -> ProbeNote {
    ProbeNote {
        component: component.into(),
        message: message.into(),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

/// # Safety
/// `p` must point to a NUL-terminated UTF-16 string.
unsafe fn from_wide_ptr(p: *const u16) -> String {
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    from_wide(std::slice::from_raw_parts(p, len))
}

pub(crate) fn is_elevated() -> bool {
    let mut token: HANDLE = null_mut();
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let token = Handle(token);
    let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut len = 0u32;
    // SAFETY: e is a TOKEN_ELEVATION of the stated size.
    let ok = unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut e as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    };
    ok != 0 && e.TokenIsElevated != 0
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}

fn open(path: &str, access: u32) -> io::Result<Handle> {
    let p = wide(path);
    // SAFETY: p is NUL-terminated; no security attributes or template.
    let h = unsafe {
        CreateFileW(
            p.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(Handle(h))
    }
}

fn ioctl(h: &Handle, code: u32, input: &[u8], out_len: usize) -> io::Result<Vec<u8>> {
    let mut out = vec![0u8; out_len];
    let mut returned = 0u32;
    let in_ptr = if input.is_empty() {
        null()
    } else {
        input.as_ptr().cast()
    };
    // SAFETY: both buffers are live for the call and their lengths are stated.
    let ok = unsafe {
        DeviceIoControl(
            h.0,
            code,
            in_ptr,
            input.len() as u32,
            out.as_mut_ptr().cast(),
            out.len() as u32,
            &mut returned,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    out.truncate(returned as usize);
    Ok(out)
}

struct DevInfo(HDEVINFO);

impl DevInfo {
    fn new(class: &windows_sys::core::GUID, flags: u32) -> Option<Self> {
        // SAFETY: no enumerator or parent window.
        let set = unsafe { SetupDiGetClassDevsW(class, null(), null_mut(), flags) };
        (set != INVALID_HANDLE_VALUE as HDEVINFO).then_some(DevInfo(set))
    }
}

impl Drop for DevInfo {
    fn drop(&mut self) {
        // SAFETY: the set is owned and destroyed exactly once.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

/// Device paths of every present interface of `class`.
fn interface_paths(class: &windows_sys::core::GUID) -> Result<Vec<String>, String> {
    let set = DevInfo::new(class, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE)
        .ok_or_else(|| io::Error::last_os_error().to_string())?;
    let mut paths = Vec::new();
    for i in 0.. {
        // SAFETY: plain data; cbSize is set before use.
        let mut ifd: SP_DEVICE_INTERFACE_DATA = unsafe { zeroed() };
        ifd.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        // SAFETY: set is live; ifd is initialised with its size.
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, null(), class, i, &mut ifd) } == 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_NO_MORE_ITEMS as i32) {
                return Err(e.to_string());
            }
            break;
        }
        let mut need = 0u32;
        // SAFETY: a null buffer asks for the required size.
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(set.0, &ifd, null_mut(), 0, &mut need, null_mut())
        };
        if (need as usize) < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() {
            continue;
        }
        let mut buf = vec![0u64; (need as usize).div_ceil(8)];
        let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: buf is at least `need` bytes and suitably aligned; cbSize is
        // the size of the fixed part, as SetupAPI requires.
        let ok = unsafe {
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            SetupDiGetDeviceInterfaceDetailW(set.0, &ifd, detail, need, null_mut(), null_mut())
        };
        if ok == 0 {
            continue;
        }
        // SAFETY: DevicePath is NUL-terminated within the `need` bytes written.
        let path = unsafe {
            let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let units = (need as usize - (start as usize - buf.as_ptr() as usize)) / 2;
            from_wide(std::slice::from_raw_parts(start, units))
        };
        paths.push(path);
    }
    Ok(paths)
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key is owned and closed exactly once.
        unsafe { RegCloseKey(self.0) };
    }
}

/// Open `HKLM\path`. `Ok(None)` when the key does not exist, which for the
/// management keys means "not enrolled", not "unknown".
fn open_key(path: &str) -> Result<Option<Key>, String> {
    let p = wide(path);
    let mut key: HKEY = null_mut();
    // SAFETY: p is NUL-terminated; key receives an owned handle on success.
    let rc = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            p.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    match rc {
        0 => Ok(Some(Key(key))),
        ERROR_FILE_NOT_FOUND => Ok(None),
        ERROR_ACCESS_DENIED => Err(format!("HKLM\\{path}: access denied")),
        e => Err(format!("HKLM\\{path}: error {e}")),
    }
}

impl Key {
    fn value(&self, name: &str) -> Option<(u32, Vec<u8>)> {
        let n = wide(name);
        let (mut kind, mut len) = (0u32, 0u32);
        // SAFETY: a null data pointer asks for the type and size.
        let rc = unsafe {
            RegQueryValueExW(self.0, n.as_ptr(), null(), &mut kind, null_mut(), &mut len)
        };
        if rc != 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        // SAFETY: buf is `len` bytes long.
        let rc = unsafe {
            RegQueryValueExW(
                self.0,
                n.as_ptr(),
                null(),
                &mut kind,
                buf.as_mut_ptr(),
                &mut len,
            )
        };
        if rc != 0 {
            return None;
        }
        buf.truncate(len as usize);
        Some((kind, buf))
    }

    fn string(&self, name: &str) -> Option<String> {
        match self.value(name)? {
            (REG_SZ | REG_EXPAND_SZ, b) => winioctl::utf16_string(&b),
            (REG_DWORD, b) if b.len() >= 4 => {
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string())
            }
            _ => None,
        }
    }

    fn binary(&self, name: &str) -> Option<Vec<u8>> {
        match self.value(name)? {
            (REG_BINARY, b) => Some(b),
            _ => None,
        }
    }

    fn subkeys(&self) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0.. {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            // SAFETY: buf holds `len` UTF-16 units; no class or timestamp requested.
            let rc = unsafe {
                RegEnumKeyExW(
                    self.0,
                    i,
                    buf.as_mut_ptr(),
                    &mut len,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if rc != 0 {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_codes_match_the_sdk() {
        assert_eq!(IOCTL_STORAGE_QUERY_PROPERTY, 0x002D_1400);
        assert_eq!(IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, 0x0007_00A0);
        assert_eq!(SMART_RCV_DRIVE_DATA, 0x0007_C088);
        assert_eq!(IOCTL_BATTERY_QUERY_TAG, 0x0029_4040);
        assert_eq!(IOCTL_BATTERY_QUERY_INFORMATION, 0x0029_4044);
    }

    #[test]
    fn recognises_enrolment_guids() {
        assert!(is_guid("{6E4D4C3A-1B2C-4D5E-8F90-A1B2C3D4E5F6}"));
        assert!(is_guid("6e4d4c3a-1b2c-4d5e-8f90-a1b2c3d4e5f6"));
        for s in ["Context", "Ownership", "ValidNodePaths", "Status"] {
            assert!(!is_guid(s));
        }
    }

    /// Runs against the real machine on the Windows CI runner: it must not
    /// panic, and it must never report a missing read as a value.
    #[test]
    fn live_scan_completes() {
        let scan = WindowsBackend::system().scan();
        assert_eq!(scan.os, "windows");
        assert!(scan.encumbrance.iter().any(|s| s.id == signals::ASSET_TAG));
        for d in &scan.storage {
            if d.transport == Transport::Usb {
                assert!(matches!(d.health, DriveHealth::Unavailable { .. }));
            }
        }
    }
}
