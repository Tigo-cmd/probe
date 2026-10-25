//! Live readings for the hands-on tests: power and charging, processor
//! speed and temperature, radios, USB devices, and a read-only drive speed
//! check. A [`Scan`](crate::Scan) is a snapshot; these are sampled again
//! and again while a test runs, so they are cheap, unprivileged reads.
//!
//! Every value is optional. A platform that does not expose a reading
//! returns `None`, and the tests say so rather than guessing.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Power and charging, now.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerNow {
    /// A charger is connected.
    pub on_ac: Option<bool>,
    /// The battery is taking charge.
    pub charging: Option<bool>,
    /// Charge level, 0–100.
    pub percent: Option<f64>,
    /// Energy left, in mWh.
    pub energy_mwh: Option<u64>,
    /// Energy when full (today, not design), in mWh.
    pub full_mwh: Option<u64>,
    /// Power into (+) or out of (−) the battery, in mW.
    pub rate_mw: Option<i64>,
    /// The charger's rating, when it says (USB-C PD, Apple adapters).
    pub adapter_watts: Option<u32>,
    pub source: String,
}

/// Processor speed and heat, now.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CpuNow {
    /// Average current clock across cores, in MHz.
    pub mhz: Option<u32>,
    /// Rated maximum clock, in MHz.
    pub max_mhz: Option<u32>,
    /// Hottest processor sensor, in °C.
    pub temp_c: Option<f32>,
    /// Speed limit the system has imposed, in percent (100 = none).
    pub limit_pct: Option<u8>,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadioKind {
    Wifi,
    Bluetooth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Radio {
    pub kind: RadioKind,
    pub name: String,
    /// Switched on (not blocked by a switch or airplane mode).
    pub enabled: Option<bool>,
    /// Wi-Fi joined to a network.
    pub connected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbDevice {
    /// Stable for as long as the device stays plugged in.
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadSpeed {
    pub mb_per_s: f64,
    pub bytes: u64,
    pub files: usize,
    pub seconds: f64,
    /// How the operating system cache was kept out of the measurement.
    pub method: String,
}

pub fn power_now() -> PowerNow {
    imp::power_now()
}

pub fn cpu_now() -> CpuNow {
    imp::cpu_now()
}

pub fn radios() -> Vec<Radio> {
    imp::radios()
}

pub fn usb_devices() -> Vec<UsbDevice> {
    imp::usb_devices()
}

// ------------------------------------------------------------- read speed

/// Read existing system files with the cache bypassed and time it. Nothing
/// is written. Stops at `max_bytes` or `max_time`, whichever comes first.
pub fn read_speed(max_bytes: u64, max_time: Duration) -> Result<ReadSpeed, String> {
    let files = largest_files(&imp::READ_ROOTS, 4_000, 48);
    if files.is_empty() {
        return Err("no readable system files found to time".into());
    }
    let mut buf = AlignedBuf::new(1 << 20);
    let start = Instant::now();
    let mut bytes = 0u64;
    let mut used = 0;
    for (path, _) in &files {
        if bytes >= max_bytes || start.elapsed() >= max_time {
            break;
        }
        let Ok(mut f) = imp::open_uncached(path) else {
            continue;
        };
        used += 1;
        loop {
            match f.read(buf.as_mut()) {
                Ok(0) => break,
                Ok(n) => bytes += n as u64,
                Err(_) => break,
            }
            if bytes >= max_bytes || start.elapsed() >= max_time {
                break;
            }
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    if bytes < 16 << 20 || seconds <= 0.0 {
        return Err("too little could be read to time the drive".into());
    }
    Ok(ReadSpeed {
        mb_per_s: bytes as f64 / 1e6 / seconds,
        bytes,
        files: used,
        seconds,
        method: imp::UNCACHED_METHOD.into(),
    })
}

/// The largest regular files under `roots`, biggest first, visiting at most
/// `budget` directory entries so a huge tree cannot stall the test.
pub fn largest_files(roots: &[&str], budget: usize, keep: usize) -> Vec<(PathBuf, u64)> {
    let mut seen = 0usize;
    let mut out: Vec<(PathBuf, u64)> = Vec::new();
    let mut stack: Vec<(PathBuf, u8)> = roots.iter().map(|r| (PathBuf::from(r), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            seen += 1;
            if seen > budget {
                break;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() && depth < 4 {
                stack.push((entry.path(), depth + 1));
            } else if ft.is_file() {
                if let Ok(m) = entry.metadata() {
                    if m.len() >= 4 << 20 {
                        out.push((entry.path(), m.len()));
                    }
                }
            }
        }
        if seen > budget {
            break;
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.1));
    out.truncate(keep);
    out
}

/// A page-aligned buffer: unbuffered reads on Windows require alignment.
struct AlignedBuf {
    ptr: *mut u8,
    layout: std::alloc::Layout,
}

impl AlignedBuf {
    fn new(len: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(len, 4096).expect("valid layout");
        // SAFETY: the layout has a non-zero size.
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "out of memory");
        AlignedBuf { ptr, layout }
    }
    fn as_mut(&mut self) -> &mut [u8] {
        // SAFETY: ptr is valid for layout.size() bytes for the buffer's life.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: allocated with this layout in `new`.
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}

#[allow(dead_code)]
fn plain_open(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

// ------------------------------------------------------- per-OS readings

#[cfg(target_os = "linux")]
use crate::backend::live_linux as imp;
#[cfg(target_os = "macos")]
use crate::backend::live_macos::sys as imp;
#[cfg(windows)]
use crate::backend::windows::live as imp;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod imp {
    use super::*;
    pub const READ_ROOTS: [&str; 0] = [];
    pub const UNCACHED_METHOD: &str = "none";
    pub fn power_now() -> PowerNow {
        PowerNow::default()
    }
    pub fn cpu_now() -> CpuNow {
        CpuNow::default()
    }
    pub fn radios() -> Vec<Radio> {
        Vec::new()
    }
    pub fn usb_devices() -> Vec<UsbDevice> {
        Vec::new()
    }
    pub fn open_uncached(path: &Path) -> std::io::Result<File> {
        plain_open(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_large_files_within_its_budget() {
        let dir = std::env::temp_dir().join(format!("hwprobe-live-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("small"), b"x").unwrap();
        std::fs::write(dir.join("sub/big"), vec![0u8; 5 << 20]).unwrap();
        let root = dir.to_string_lossy().into_owned();
        let found = largest_files(&[root.as_str()], 100, 10);
        assert_eq!(found.len(), 1, "files under 4 MB are ignored");
        assert!(found[0].0.ends_with("sub/big"));
        assert!(largest_files(&[root.as_str()], 1, 10).len() <= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_reads_never_panic_here() {
        let _ = power_now();
        let _ = cpu_now();
        let _ = radios();
        let _ = usb_devices();
    }
}
