//! Desktop shell for hwprobe.
//!
//! The commands are thin wrappers over plain functions so the behaviour is
//! tested without a webview. All grading happens here, in the core crate: the
//! frontend only presents what it is given. A saved scan is always re-graded
//! on open, so its verdicts come from the grader this build ships with.
//!
//! Every scan the app runs or opens is kept in a local history (scanstore).
//! If the history cannot be opened the app still scans and reports; it says
//! why the history is unavailable instead of failing.

pub mod privilege;
pub mod team;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hwprobe::fingerprint::Divergence;
use hwprobe::grade::{self, Grade};
use hwprobe::{assess, assess_with, report, Assessment, Fingerprint, HandsOn, Scan};
use scanstore::{Origin, QueueCounts, Store, Summary};
use serde::Serialize;
use tauri::Manager;

/// A scan with its grade and assessment, as the frontend receives it.
#[derive(Debug, Clone, Serialize)]
pub struct Graded {
    pub scan: Scan,
    pub grade: Grade,
    /// Condition score and fit for each kind of use, computed from the grade.
    pub assessment: Assessment,
}

pub fn graded(scan: Scan) -> Graded {
    let grade = grade::grade(&scan);
    let assessment = assess(&scan, &grade);
    Graded {
        scan,
        grade,
        assessment,
    }
}

/// A graded scan plus its place in the history.
#[derive(Debug, Clone, Serialize)]
pub struct Opened {
    pub scan: Scan,
    pub grade: Grade,
    pub assessment: Assessment,
    /// Hands-on test results recorded for this scan, if any.
    pub hands_on: Option<HandsOn>,
    /// The history row for this scan; `None` when the history is unavailable.
    pub record: Option<Summary>,
    /// Other stored scans that claim to be the same machine, newest first.
    pub earlier: Vec<Summary>,
    pub history_error: Option<String>,
}

/// Store `scan` (a no-op if it is already stored) and find its relatives.
pub fn record(store: &mut Store, scan: Scan, origin: Origin) -> Result<Opened, String> {
    let stored = store.insert(&scan, origin).map_err(|e| e.to_string())?;
    opened(store, scan, stored.id)
}

fn opened(store: &Store, scan: Scan, id: i64) -> Result<Opened, String> {
    let record = store.summary(id).map_err(|e| e.to_string())?;
    let earlier = store.same_machine(&scan).map_err(|e| e.to_string())?;
    let hands_on = store.hands_on(id).map_err(|e| e.to_string())?;
    let grade = grade::grade(&scan);
    let assessment = assess_with(&scan, &grade, hands_on.as_ref());
    Ok(Opened {
        scan,
        grade,
        assessment,
        hands_on,
        record: Some(record),
        earlier,
        history_error: None,
    })
}

fn unrecorded(scan: Scan, error: String) -> Opened {
    let Graded {
        scan,
        grade,
        assessment,
    } = graded(scan);
    Opened {
        scan,
        grade,
        assessment,
        hands_on: None,
        record: None,
        earlier: Vec::new(),
        history_error: Some(error),
    }
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

/// The history list, as the home screen shows it.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryView {
    pub scans: Vec<Summary>,
    pub counts: QueueCounts,
    /// Where the history file lives, so a technician can back it up.
    pub location: String,
    pub error: Option<String>,
}

/// The app's history: the open store, or why it could not be opened.
pub struct History {
    store: Mutex<Result<Store, String>>,
    location: PathBuf,
}

impl History {
    pub fn open(location: PathBuf) -> Self {
        let store = location
            .parent()
            .map(fs::create_dir_all)
            .transpose()
            .map_err(|e| format!("{}: {e}", location.display()))
            .and_then(|_| Store::open(&location).map_err(|e| e.to_string()));
        History {
            store: Mutex::new(store),
            location,
        }
    }

    pub fn unavailable(error: String) -> Self {
        History {
            store: Mutex::new(Err(error)),
            location: PathBuf::new(),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| "scan history is unavailable after an earlier error".to_string())?;
        match guard.as_mut() {
            Ok(store) => f(store),
            Err(e) => Err(format!("scan history unavailable: {e}")),
        }
    }

    /// Store and annotate a scan; on any history failure still return the graded scan.
    fn keep(&self, scan: Scan, origin: Origin) -> Opened {
        let attempt = scan.clone();
        self.with(move |s| record(s, attempt, origin))
            .unwrap_or_else(|e| unrecorded(scan, e))
    }

    fn view(&self) -> HistoryView {
        let location = self.location.display().to_string();
        match self.with(|s| {
            Ok((
                s.list().map_err(|e| e.to_string())?,
                s.queue_counts().map_err(|e| e.to_string())?,
            ))
        }) {
            Ok((scans, counts)) => HistoryView {
                scans,
                counts,
                location,
                error: None,
            },
            Err(e) => HistoryView {
                scans: Vec::new(),
                counts: QueueCounts::default(),
                location,
                error: Some(e),
            },
        }
    }
}

#[tauri::command]
async fn run_scan(history: tauri::State<'_, History>) -> Result<Opened, String> {
    // Drive and battery reads block; keep them off the UI thread.
    let scan = tauri::async_runtime::spawn_blocking(hwprobe::scan)
        .await
        .map_err(|e| format!("scan failed: {e}"))?;
    Ok(history.keep(scan, Origin::Live))
}

/// Scan with full access: directly when already elevated, otherwise through
/// the OS's administrator prompt (see [`privilege`]).
#[tauri::command]
async fn run_elevated_scan(
    history: tauri::State<'_, History>,
) -> Result<Opened, privilege::ElevatedError> {
    let scan = tauri::async_runtime::spawn_blocking(privilege::scan_with_privilege)
        .await
        .map_err(|e| privilege::ElevatedError {
            kind: "failed",
            message: format!("scan failed: {e}"),
        })??;
    Ok(history.keep(scan, Origin::Live))
}

#[tauri::command]
fn privilege_status() -> privilege::Privilege {
    privilege::privilege()
}

#[tauri::command]
fn open_scan(history: tauri::State<'_, History>, path: String) -> Result<Opened, String> {
    load(Path::new(&path)).map(|scan| history.keep(scan, Origin::Imported))
}

#[tauri::command]
fn list_history(history: tauri::State<'_, History>) -> HistoryView {
    history.view()
}

#[tauri::command]
fn open_stored(history: tauri::State<'_, History>, id: i64) -> Result<Opened, String> {
    history.with(|s| {
        let (_, scan) = s.get(id).map_err(|e| e.to_string())?;
        opened(s, scan, id)
    })
}

#[tauri::command]
fn delete_stored(history: tauri::State<'_, History>, id: i64) -> Result<(), String> {
    history.with(|s| s.delete(id).map_err(|e| e.to_string()))
}

#[tauri::command]
fn label_stored(
    history: tauri::State<'_, History>,
    id: i64,
    label: Option<String>,
) -> Result<Summary, String> {
    history.with(|s| {
        s.set_label(id, label.as_deref())
            .map_err(|e| e.to_string())?;
        s.summary(id).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn compare_stored(
    history: tauri::State<'_, History>,
    id: i64,
    scan: Scan,
) -> Result<Comparison, String> {
    history.with(|s| {
        let (_, earlier) = s.get(id).map_err(|e| e.to_string())?;
        Ok(compare(&earlier, &scan))
    })
}

#[tauri::command]
fn save_scan(path: String, scan: Scan) -> Result<(), String> {
    save(Path::new(&path), &scan)
}

#[tauri::command]
fn save_text_report(
    history: tauri::State<'_, History>,
    path: String,
    scan: Scan,
    id: Option<i64>,
) -> Result<(), String> {
    let g = grade::grade(&scan);
    // Include hands-on results when the scan is in the history; a missing
    // history only means the report goes without them.
    let hands_on = id.and_then(|id| {
        history
            .with(|s| s.hands_on(id).map_err(|e| e.to_string()))
            .ok()
            .flatten()
    });
    fs::write(&path, report::render_with(&scan, &g, hands_on.as_ref()))
        .map_err(|e| format!("{path}: {e}"))
}

/// Record one finished hands-on test against a stored scan and return the
/// scan re-assessed with it.
pub fn record_hands_on(
    store: &mut Store,
    id: i64,
    result: hwprobe::handson::TestResult,
) -> Result<Opened, String> {
    let mut h = store
        .hands_on(id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    h.record(result);
    store.set_hands_on(id, &h).map_err(|e| e.to_string())?;
    let (_, scan) = store.get(id).map_err(|e| e.to_string())?;
    opened(store, scan, id)
}

#[tauri::command]
fn save_hands_on_result(
    history: tauri::State<'_, History>,
    id: i64,
    result: hwprobe::handson::TestResult,
) -> Result<Opened, String> {
    history.with(|s| record_hands_on(s, id, result))
}

#[tauri::command]
fn compare_with(path: String, scan: Scan) -> Result<Comparison, String> {
    load(Path::new(&path)).map(|earlier| compare(&earlier, &scan))
}

// ------------------------------------------------------- live readings

#[tauri::command]
fn live_power() -> hwprobe::live::PowerNow {
    hwprobe::live::power_now()
}

#[tauri::command]
fn live_cpu() -> hwprobe::live::CpuNow {
    hwprobe::live::cpu_now()
}

#[tauri::command]
async fn live_radios() -> Result<Vec<hwprobe::live::Radio>, String> {
    // system_profiler can take a few seconds on a Mac.
    tauri::async_runtime::spawn_blocking(hwprobe::live::radios)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn live_usb() -> Result<Vec<hwprobe::live::UsbDevice>, String> {
    tauri::async_runtime::spawn_blocking(hwprobe::live::usb_devices)
        .await
        .map_err(|e| e.to_string())
}

/// Read-only: times reading existing system files with the cache bypassed.
#[tauri::command]
async fn read_speed() -> Result<hwprobe::live::ReadSpeed, String> {
    tauri::async_runtime::spawn_blocking(|| {
        hwprobe::live::read_speed(1 << 30, std::time::Duration::from_secs(12))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Let the app's own window use the camera and microphone on Linux, where
/// WebKitGTK refuses media capture by default. Only media requests are
/// granted; any other permission request keeps the default answer.
#[cfg(target_os = "linux")]
fn allow_media_capture(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.with_webview(|wv| {
        use webkit2gtk::glib::prelude::Cast;
        use webkit2gtk::{
            PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, WebViewExt,
        };
        let view = wv.inner();
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_media_stream(true);
        }
        view.connect_permission_request(|_, request| {
            if request
                .downcast_ref::<UserMediaPermissionRequest>()
                .is_some()
            {
                request.allow();
                true
            } else {
                false
            }
        });
    });
}

// ------------------------------------------------------------ team sharing

#[tauri::command]
fn team_status(team: tauri::State<'_, team::Team>) -> team::TeamStatus {
    team.status()
}

#[tauri::command]
async fn team_connect(
    app: tauri::AppHandle,
    server: String,
    token: String,
) -> Result<team::TeamStatus, String> {
    // Network calls block; keep them off the UI thread.
    tauri::async_runtime::spawn_blocking(move || app.state::<team::Team>().connect(&server, &token))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn team_disconnect(team: tauri::State<'_, team::Team>) -> Result<team::TeamStatus, String> {
    team.disconnect()
}

#[tauri::command]
async fn share_stored(app: tauri::AppHandle, id: i64) -> Result<team::Shared, String> {
    tauri::async_runtime::spawn_blocking(move || {
        team::share(&app.state::<team::Team>(), &app.state::<History>(), id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn withdraw_stored(history: tauri::State<'_, History>, id: i64) -> Result<Summary, String> {
    history.with(|s| {
        s.withdraw(id).map_err(|e| e.to_string())?;
        s.summary(id).map_err(|e| e.to_string())
    })
}

#[tauri::command]
async fn sync_now(app: tauri::AppHandle) -> Result<syncclient::DrainReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<team::Team>().sync(&app.state::<History>())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn team_lookup(
    app: tauri::AppHandle,
    scan: Scan,
    exclude: Option<String>,
) -> Result<syncclient::Lookup, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<team::Team>().lookup(&scan, exclude.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir();
            let history = match &data {
                Ok(dir) => History::open(dir.join("scans.sqlite3")),
                Err(e) => History::unavailable(format!("no app data folder: {e}")),
            };
            let team = team::Team::open(
                data.map(|d| d.join("sync.json"))
                    .unwrap_or_else(|_| std::env::temp_dir().join("probe-sync.json")),
            );
            let retry = team.status().configured;
            app.manage(history);
            app.manage(team);
            #[cfg(target_os = "linux")]
            allow_media_capture(app);
            // Retry anything left queued from an earlier session, quietly:
            // failures stay in the history and show on the scans they affect.
            if retry {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = handle
                        .state::<team::Team>()
                        .sync(&handle.state::<History>());
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_scan,
            run_elevated_scan,
            privilege_status,
            open_scan,
            list_history,
            open_stored,
            delete_stored,
            label_stored,
            compare_stored,
            save_scan,
            save_text_report,
            compare_with,
            save_hands_on_result,
            live_power,
            live_cpu,
            live_radios,
            live_usb,
            read_speed,
            team_status,
            team_connect,
            team_disconnect,
            share_stored,
            withdraw_stored,
            sync_now,
            team_lookup
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
    fn a_hands_on_result_is_kept_and_changes_the_assessment() {
        use hwprobe::handson::{Outcome, TestKind, TestResult};
        let mut store = Store::open_in_memory().unwrap();
        let first = record(&mut store, sample(), Origin::Live).unwrap();
        let id = first.record.as_ref().unwrap().id;
        assert!(first.hands_on.is_none());
        let fail = TestResult {
            kind: TestKind::Screen,
            outcome: Outcome::Problems,
            tested_at: 1,
            summary: "Dead or stuck pixels".into(),
            problems: vec!["Dead or stuck pixels".into()],
            detail: serde_json::Value::Null,
        };
        let after = record_hands_on(&mut store, id, fail).unwrap();
        assert_eq!(after.hands_on.as_ref().unwrap().tests.len(), 1);
        let summary = after.assessment.hands_on.as_ref().unwrap();
        assert_eq!(summary.problems, ["Screen: Dead or stuck pixels"]);
        // Reopening from the history shows the same result.
        let (_, scan) = store.get(id).unwrap();
        assert_eq!(opened(&store, scan, id).unwrap().hands_on, after.hands_on);
        assert!(
            record_hands_on(&mut store, 999, after.hands_on.unwrap().tests[0].clone()).is_err()
        );
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
    const DEMO_OPENED: &str = "../src/lib/fixtures/demo.json";
    const DEMO_HISTORY: &str = "../src/lib/fixtures/demo-history.json";
    const DEMO_COMPARE: &str = "../src/lib/fixtures/demo-compare.json";

    /// The design-preview payloads, produced by the real store and grader: the
    /// demo laptop, an earlier scan of it with a different drive, and another
    /// laptop that is still enrolled in management.
    fn demo_payloads() -> (Opened, HistoryView, Comparison) {
        let current: Scan = serde_json::from_str(DEMO_SCAN).expect("demo-scan.json is a scan");
        let mut earlier = current.clone();
        earlier.started_at -= 14 * 86_400;
        earlier.storage[0].serial = Field::claimed(Some("S4EWNX0R998877".into()), "demo");
        let mut other = current.clone();
        other.started_at -= 3 * 86_400;
        other.identity.vendor = Field::claimed(Some("Dell Inc.".into()), "demo");
        other.identity.model = Field::claimed(Some("Latitude 7490".into()), "demo");
        other.identity.serial = Field::claimed(Some("8JQ1KX2".into()), "demo");
        other.identity.board_serial = Field::claimed(Some("/8JQ1KX2/CN129638A1".into()), "demo");
        other.identity.uuid =
            Field::claimed(Some("4c4c4544-004a-5110-8031-b8c04f4b5832".into()), "demo");
        if let Some(mdm) = other
            .encumbrance
            .iter_mut()
            .find(|s| s.id == "mdm_enrolment")
        {
            mdm.present = true;
            mdm.detail = "enrolled in device management (MS DM Server for contoso.com): the organisation can lock or wipe it".into();
        }

        let comparison = compare(&earlier, &current);
        let mut store = Store::open_in_memory().unwrap();
        store.insert(&earlier, Origin::Live).unwrap();
        let other_id = store.insert(&other, Origin::Imported).unwrap().id;
        store
            .set_label(other_id, Some("Marketplace listing, seller in Leeds"))
            .unwrap();
        let mut opened = record(&mut store, current, Origin::Live).unwrap();
        let mut view = HistoryView {
            scans: store.list().unwrap(),
            counts: store.queue_counts().unwrap(),
            location: "~/.local/share/app.probe.desktop/scans.sqlite3".into(),
            error: None,
        };
        // Storage time is wall-clock; pin it so the fixtures are reproducible.
        let pin = |s: &mut Summary| s.stored_at = s.started_at;
        opened.record.iter_mut().for_each(pin);
        opened.earlier.iter_mut().for_each(pin);
        view.scans.iter_mut().for_each(pin);
        (opened, view, comparison)
    }

    fn json_file(path: &str) -> serde_json::Value {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// The design-preview fixtures must be exactly what this code produces.
    #[test]
    fn demo_fixtures_match_the_code() {
        let (opened, view, comparison) = demo_payloads();
        let stale = "is stale: run `cargo test -- --ignored regenerate_demo_fixtures`";
        assert_eq!(
            json_file(DEMO_OPENED),
            serde_json::to_value(&opened).unwrap(),
            "demo.json {stale}"
        );
        assert_eq!(
            json_file(DEMO_HISTORY),
            serde_json::to_value(&view).unwrap(),
            "demo-history.json {stale}"
        );
        assert_eq!(
            json_file(DEMO_COMPARE),
            serde_json::to_value(&comparison).unwrap(),
            "demo-compare.json {stale}"
        );
        assert_eq!(
            opened.earlier.len(),
            1,
            "only the same laptop is offered for comparison"
        );
        assert_eq!(comparison.divergences.len(), 2, "the swapped drive shows");
        assert_eq!(
            opened.earlier.len(),
            1,
            "only the same laptop is offered for comparison"
        );
    }

    #[test]
    #[ignore = "writes the demo fixtures"]
    fn regenerate_demo_fixtures() {
        let (opened, view, comparison) = demo_payloads();
        fs::write(
            DEMO_COMPARE,
            serde_json::to_string_pretty(&comparison).unwrap() + "\n",
        )
        .unwrap();
        fs::write(
            DEMO_OPENED,
            serde_json::to_string_pretty(&opened).unwrap() + "\n",
        )
        .unwrap();
        fs::write(
            DEMO_HISTORY,
            serde_json::to_string_pretty(&view).unwrap() + "\n",
        )
        .unwrap();
    }

    #[test]
    fn scans_are_kept_and_matched_to_earlier_scans_of_the_machine() {
        let mut store = Store::open_in_memory().unwrap();
        let first = record(&mut store, sample(), Origin::Live).unwrap();
        assert!(first.earlier.is_empty());
        assert_eq!(first.record.as_ref().unwrap().origin, Origin::Live);

        let mut later = sample();
        later.started_at += 86_400;
        later.storage[0].serial = Field::claimed(Some("WD-WX11A0000".into()), "test");
        let second = record(&mut store, later.clone(), Origin::Imported).unwrap();
        assert_eq!(second.earlier.len(), 1);
        let earlier_id = second.earlier[0].id;
        let (_, earlier) = store.get(earlier_id).unwrap();
        assert_eq!(compare(&earlier, &later).divergences.len(), 2);

        let again = record(&mut store, sample(), Origin::Imported).unwrap();
        assert_eq!(
            again.record.unwrap().id,
            first.record.unwrap().id,
            "re-opening does not duplicate"
        );
    }

    #[test]
    fn an_unavailable_history_never_blocks_a_scan() {
        let h = History::unavailable("disk full".into());
        let opened = h.keep(sample(), Origin::Live);
        assert!(opened.record.is_none());
        assert!(opened.history_error.unwrap().contains("disk full"));
        assert_eq!(opened.grade.grader_version, grade::GRADER_VERSION);
        assert!(h.view().error.is_some());

        let dir = std::env::temp_dir().join(format!("probe-desktop-hist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let h = History::open(dir.join("nested/scans.sqlite3"));
        assert!(
            h.keep(sample(), Origin::Live).record.is_some(),
            "creates its folder"
        );
        assert_eq!(h.view().scans.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn graded_payload_serialises_for_the_frontend() {
        let v = serde_json::to_value(graded(sample())).unwrap();
        assert!(v["grade"]["axes"].is_array());
        assert_eq!(v["scan"]["os"], "linux");
    }
}
