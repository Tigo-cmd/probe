//! Full scans with administrator rights, without running the app as admin.
//!
//! The app never elevates itself. To scan with full access it asks the OS to
//! start a second copy of its own binary with administrator rights and one
//! argument: `--elevated-scan <file>`. That copy runs the scan, writes the
//! JSON to `<file>` and exits before any window or web view exists. The app,
//! still unprivileged, reads the file and carries on.
//!
//! The hand-off file lives in a fresh owner-only directory the app created
//! (see [`elevate::PrivateDir`]). The elevated side refuses any path that is
//! not shaped like one, and never overwrites: it creates the file or fails.

use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use elevate::{Outcome, PrivateDir};
use hwprobe::Scan;
use serde::Serialize;

/// The only argument the elevated copy accepts.
pub const FLAG: &str = "--elevated-scan";
const DIR_PREFIX: &str = "probe-elevated-";
const FILE_NAME: &str = "scan.json";

/// Exit statuses of the elevated copy (sysexits-style).
pub const EXIT_OK: i32 = 0;
pub const EXIT_BAD_ARGS: i32 = 64;
pub const EXIT_WRITE_FAILED: i32 = 73;

/// What the home screen needs to offer the right scan button.
#[derive(Debug, Clone, Serialize)]
pub struct Privilege {
    /// Already running with full access: no prompt needed.
    pub elevated: bool,
    /// A prompt can be raised on this machine.
    pub can_elevate: bool,
    /// What the person will see, e.g. "macOS administrator password".
    pub method: String,
    /// Why a prompt cannot be raised, when it cannot.
    pub reason: Option<String>,
}

pub fn privilege() -> Privilege {
    let elevated = hwprobe::is_elevated();
    let available = elevate::available();
    Privilege {
        elevated,
        can_elevate: !elevated && available.is_ok(),
        method: elevate::method().to_string(),
        reason: available.err(),
    }
}

/// Why a full-access scan did not happen. `kind` lets the screen tell a
/// dismissed prompt apart from a real failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ElevatedError {
    pub kind: &'static str,
    pub message: String,
}

impl ElevatedError {
    fn cancelled() -> Self {
        ElevatedError {
            kind: "cancelled",
            message: "Administrator access was not given.".into(),
        }
    }
    fn unavailable(message: impl Into<String>) -> Self {
        ElevatedError {
            kind: "unavailable",
            message: message.into(),
        }
    }
    fn failed(message: impl Into<String>) -> Self {
        ElevatedError {
            kind: "failed",
            message: message.into(),
        }
    }
}

/// Entry point for the elevated copy. `None` when the arguments are not a
/// hand-off request, so the app starts normally; otherwise the exit status.
pub fn handoff_entry(args: &[OsString]) -> Option<i32> {
    if args.first().map(OsString::as_os_str) != Some(OsStr::new(FLAG)) {
        return None;
    }
    Some(match args {
        [_, path] => handoff(Path::new(path), hwprobe::scan),
        _ => EXIT_BAD_ARGS,
    })
}

fn handoff(path: &Path, scan: impl FnOnce() -> Scan) -> i32 {
    if !elevate::is_handoff_path(path, DIR_PREFIX, FILE_NAME) {
        return EXIT_BAD_ARGS;
    }
    write_new(path, &scan())
}

/// Create `path` and write the scan. Never follows or replaces an existing file.
fn write_new(path: &Path, scan: &Scan) -> i32 {
    let Ok(json) = serde_json::to_vec(scan) else {
        return EXIT_WRITE_FAILED;
    };
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o644);
    match options.open(path).and_then(|mut f| f.write_all(&json)) {
        Ok(()) => EXIT_OK,
        Err(_) => EXIT_WRITE_FAILED,
    }
}

/// Scan with full access: directly if already elevated, otherwise through
/// the OS prompt and a hand-off file.
pub fn scan_with_privilege() -> Result<Scan, ElevatedError> {
    if hwprobe::is_elevated() {
        return Ok(hwprobe::scan());
    }
    let exe = std::env::current_exe()
        .map_err(|e| ElevatedError::failed(format!("cannot locate the probe program: {e}")))?;
    let dir = PrivateDir::create(DIR_PREFIX)
        .map_err(|e| ElevatedError::failed(format!("cannot create a private folder: {e}")))?;
    let out = dir.path().join(FILE_NAME);
    let outcome = elevate::run(&exe, &[OsStr::new(FLAG), out.as_os_str()]);
    interpret(outcome, &out)
    // `dir` drops here and removes the hand-off file.
}

/// Turn the prompt's outcome and the hand-off file into a scan or a reason.
pub fn interpret(outcome: Outcome, out: &Path) -> Result<Scan, ElevatedError> {
    match outcome {
        Outcome::Exited(EXIT_OK) => {
            let text = std::fs::read_to_string(out).map_err(|e| {
                ElevatedError::failed(format!("the elevated scan left no result: {e}"))
            })?;
            serde_json::from_str(&text).map_err(|e| {
                ElevatedError::failed(format!("the elevated scan's result is unreadable: {e}"))
            })
        }
        Outcome::Exited(EXIT_BAD_ARGS) => Err(ElevatedError::failed(
            "the elevated scan refused its arguments",
        )),
        Outcome::Exited(EXIT_WRITE_FAILED) => Err(ElevatedError::failed(
            "the elevated scan could not write its result",
        )),
        Outcome::Exited(code) => Err(ElevatedError::failed(format!(
            "the elevated scan stopped with status {code}"
        ))),
        Outcome::Cancelled => Err(ElevatedError::cancelled()),
        Outcome::Unavailable(why) => Err(ElevatedError::unavailable(why)),
        Outcome::Failed(why) => Err(ElevatedError::failed(why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scan {
        Scan {
            os: "linux".into(),
            elevated: true,
            started_at: 1_700_000_000,
            ..Scan::default()
        }
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn normal_launches_are_not_handoffs() {
        assert_eq!(handoff_entry(&os(&[])), None);
        assert_eq!(handoff_entry(&os(&["--something"])), None);
        assert_eq!(handoff_entry(&os(&[FLAG])), Some(EXIT_BAD_ARGS));
        assert_eq!(handoff_entry(&os(&[FLAG, "a", "b"])), Some(EXIT_BAD_ARGS));
    }

    #[test]
    fn writes_only_to_a_handoff_path_and_never_overwrites() {
        let dir = PrivateDir::create(DIR_PREFIX).unwrap();
        let out = dir.path().join(FILE_NAME);

        let stray = dir.path().join("elsewhere.json");
        assert_eq!(handoff(&stray, sample), EXIT_BAD_ARGS);
        assert!(!stray.exists(), "a refused path is never written");

        assert_eq!(handoff(&out, sample), EXIT_OK);
        assert_eq!(interpret(Outcome::Exited(EXIT_OK), &out).unwrap(), sample());

        assert_eq!(
            handoff(&out, sample),
            EXIT_WRITE_FAILED,
            "an existing file is never replaced"
        );
    }

    #[test]
    fn says_why_a_full_scan_did_not_happen() {
        let missing = std::env::temp_dir().join("probe-elevated-none/scan.json");
        let kind = |o| interpret(o, &missing).unwrap_err().kind;
        assert_eq!(kind(Outcome::Cancelled), "cancelled");
        assert_eq!(
            kind(Outcome::Unavailable("no polkit agent".into())),
            "unavailable"
        );
        assert_eq!(kind(Outcome::Failed("boom".into())), "failed");
        assert_eq!(kind(Outcome::Exited(EXIT_BAD_ARGS)), "failed");
        assert!(interpret(Outcome::Exited(EXIT_OK), &missing)
            .unwrap_err()
            .message
            .contains("no result"));
    }

    #[test]
    fn privilege_is_consistent() {
        let p = privilege();
        assert!(
            !(p.elevated && p.can_elevate),
            "never offer a prompt when already elevated"
        );
        assert!(!p.method.is_empty());
    }
}
