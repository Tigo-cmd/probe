//! Desktop shell for hwprobe.
//!
//! The commands are thin wrappers over plain functions so the behaviour is
//! tested without a webview. All grading happens here, in the core crate: the
//! frontend only presents what it is given. A saved scan is always re-graded
//! on open, so its verdicts come from the grader this build ships with.

use std::fs;
use std::path::Path;

use hwprobe::fingerprint::Divergence;
use hwprobe::grade::{self, Grade};
use hwprobe::{report, Fingerprint, Scan};
use serde::Serialize;

/// A scan with its grade, as the frontend receives it.
#[derive(Debug, Clone, Serialize)]
pub struct Graded {
    pub scan: Scan,
    pub grade: Grade,
}

pub fn graded(scan: Scan) -> Graded {
    let grade = grade::grade(&scan);
    Graded { scan, grade }
}

pub fn load(path: &Path) -> Result<Scan, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not a saved scan: {e}", path.display()))
}

pub fn save(path: &Path, scan: &Scan) -> Result<(), String> {
    let text = serde_json::to_string_pretty(scan).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// How two scans of what should be the same machine relate.
#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    /// Identifiers present in both scans that match.
    pub matching: usize,
    /// Identifiers present in both scans that differ: the tamper signal.
    pub divergences: Vec<Divergence>,
    pub earlier_started_at: u64,
}

pub fn compare(earlier: &Scan, current: &Scan) -> Comparison {
    let (a, b) = (Fingerprint::of(earlier), Fingerprint::of(current));
    Comparison {
        matching: a.matching(&b),
        divergences: a.diverges_from(&b),
        earlier_started_at: earlier.started_at,
    }
}

#[tauri::command]
async fn run_scan() -> Result<Graded, String> {
    // Drive and battery reads block; keep them off the UI thread.
    tauri::async_runtime::spawn_blocking(|| graded(hwprobe::scan()))
        .await
        .map_err(|e| format!("scan failed: {e}"))
}

#[tauri::command]
fn open_scan(path: String) -> Result<Graded, String> {
    load(Path::new(&path)).map(graded)
}

#[tauri::command]
fn save_scan(path: String, scan: Scan) -> Result<(), String> {
    save(Path::new(&path), &scan)
}

#[tauri::command]
fn save_text_report(path: String, scan: Scan) -> Result<(), String> {
    let g = grade::grade(&scan);
    fs::write(&path, report::render(&scan, &g)).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn compare_with(path: String, scan: Scan) -> Result<Comparison, String> {
    load(Path::new(&path)).map(|earlier| compare(&earlier, &scan))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            run_scan,
            open_scan,
            save_scan,
            save_text_report,
            compare_with
        ])
        .run(tauri::generate_context!())
        .expect("error while running the probe desktop app");
}

#[cfg(test)]
mod tests {
    use super::*;
    use hwprobe::model::{Drive, DriveHealth, Field, Transport};

    fn sample() -> Scan {
        let mut s = Scan {
            os: "linux".into(),
            started_at: 1_700_000_000,
            ..Scan::default()
        };
        s.identity.uuid = Field::claimed(Some("4c4c4544-0001".into()), "test");
        s.storage.push(Drive {
            name: "nvme0n1".into(),
            transport: Transport::Nvme,
            rotational: Field::default(),
            removable: false,
            model: Field::default(),
            serial: Field::claimed(Some("S4EWNX0R123456".into()), "test"),
            firmware: Field::default(),
            capacity_bytes: Field::default(),
            health: DriveHealth::Unavailable {
                reason: "test".into(),
            },
        });
        s
    }

    #[test]
    fn saved_scans_round_trip_and_are_regraded() {
        let dir = std::env::temp_dir().join(format!("probe-desktop-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scan.json");
        save(&path, &sample()).unwrap();
        let g = graded(load(&path).unwrap());
        assert_eq!(g.scan, sample());
        assert_eq!(g.grade.grader_version, grade::GRADER_VERSION);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_files_that_are_not_scans() {
        let dir = std::env::temp_dir().join(format!("probe-desktop-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.json");
        fs::write(&path, "{\"hello\": 1}").unwrap();
        assert!(load(&path).unwrap_err().contains("not a saved scan"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn comparison_reports_swapped_parts() {
        let before = sample();
        let mut after = sample();
        after.storage[0].serial = Field::claimed(Some("WD-WX11A0000".into()), "test");
        let c = compare(&before, &after);
        assert_eq!(c.matching, 1);
        assert_eq!(c.divergences.len(), 2);
        assert!(compare(&before, &before).divergences.is_empty());
    }

    const DEMO_SCAN: &str = include_str!("../../src/lib/fixtures/demo-scan.json");
    const DEMO_GRADED: &str = "../src/lib/fixtures/demo.json";

    /// The design-preview fixture must be exactly what this grader produces.
    #[test]
    fn demo_fixture_matches_the_grader() {
        let scan: Scan = serde_json::from_str(DEMO_SCAN).expect("demo-scan.json is a scan");
        let expected = serde_json::to_value(graded(scan)).unwrap();
        let committed: serde_json::Value = fs::read_to_string(DEMO_GRADED)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        assert_eq!(
            committed, expected,
            "demo.json is stale: run `cargo test -- --ignored regenerate_demo_fixture`"
        );
    }

    #[test]
    #[ignore = "writes the demo fixture"]
    fn regenerate_demo_fixture() {
        let scan: Scan = serde_json::from_str(DEMO_SCAN).unwrap();
        let text = serde_json::to_string_pretty(&graded(scan)).unwrap();
        fs::write(DEMO_GRADED, text + "\n").unwrap();
    }

    #[test]
    fn graded_payload_serialises_for_the_frontend() {
        let v = serde_json::to_value(graded(sample())).unwrap();
        assert!(v["grade"]["axes"].is_array());
        assert_eq!(v["scan"]["os"], "linux");
    }
}
