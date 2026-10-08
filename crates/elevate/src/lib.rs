//! elevate: run one command with administrator rights through the operating
//! system's own prompt, wait for it, and say plainly how it ended.
//!
//! - Windows: `ShellExecuteExW` with the `runas` verb (the UAC prompt).
//! - macOS: `osascript` running `do shell script … with administrator
//!   privileges` (the system password dialog).
//! - Linux: `pkexec` (the desktop's polkit agent).
//!
//! Only a program and its arguments cross the privilege boundary. The caller
//! passes results back through a file in a [`PrivateDir`] it created, because
//! the Windows prompt gives no access to the elevated process's output.
//!
//! The quoting and outcome rules are pure functions, tested on every OS; the
//! code that actually raises a prompt is the thin `sys` part of [`run`].

use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// How an elevation attempt ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The elevated program ran and exited with this status.
    Exited(i32),
    /// The user dismissed the prompt.
    Cancelled,
    /// Elevation is not possible here (no polkit agent, not an admin, …).
    Unavailable(String),
    /// Something else went wrong; the message says what.
    Failed(String),
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Exited(c) => write!(f, "the elevated program exited with status {c}"),
            Outcome::Cancelled => write!(f, "administrator access was not given"),
            Outcome::Unavailable(why) => write!(f, "administrator access is unavailable: {why}"),
            Outcome::Failed(why) => write!(f, "elevation failed: {why}"),
        }
    }
}

/// The words a person sees for this platform's prompt.
pub fn method() -> &'static str {
    if cfg!(windows) {
        "Windows administrator prompt (UAC)"
    } else if cfg!(target_os = "macos") {
        "macOS administrator password"
    } else {
        "administrator password (polkit)"
    }
}

/// Whether a prompt can be raised here, and if not, why.
pub fn available() -> Result<(), String> {
    #[cfg(windows)]
    {
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        if Path::new(OSASCRIPT).exists() {
            Ok(())
        } else {
            Err(format!("{OSASCRIPT} is missing"))
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_available(
            std::env::var_os("APPIMAGE").is_some(),
            find_in_path("pkexec").is_some(),
        )
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err("this operating system has no supported elevation prompt".into())
    }
}

/// Run `program args…` elevated and wait for it.
pub fn run(program: &Path, args: &[&OsStr]) -> Outcome {
    if let Err(why) = available() {
        return Outcome::Unavailable(why);
    }
    sys::run(program, args)
}

// ------------------------------------------------------------------ Linux

#[cfg(all(unix, not(target_os = "macos")))]
fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join(name))
            .find(|c| c.is_file())
    })
}

/// An AppImage runs from a FUSE mount that root usually cannot read, so
/// `pkexec` would fail to start it; say so up front instead.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn linux_available(in_appimage: bool, has_pkexec: bool) -> Result<(), String> {
    if in_appimage {
        Err("an AppImage cannot be re-run as root; run the hwprobe command-line tool with sudo instead".into())
    } else if !has_pkexec {
        Err(
            "pkexec (polkit) is not installed; run the hwprobe command-line tool with sudo instead"
                .into(),
        )
    } else {
        Ok(())
    }
}

/// pkexec: 126 means the dialog was dismissed, 127 that authorisation was
/// refused or could not be obtained. Anything else is the program's own status.
pub fn classify_pkexec(status: Option<i32>, stderr: &str) -> Outcome {
    match status {
        Some(126) => Outcome::Cancelled,
        // pkexec appends "This incident has been reported." after a refusal;
        // the first line carries the reason.
        Some(127) => Outcome::Unavailable(non_empty(first_line(stderr), "not authorised")),
        Some(code) => Outcome::Exited(code),
        None => Outcome::Failed(non_empty(stderr, "pkexec was terminated by a signal")),
    }
}

// ------------------------------------------------------------------ macOS

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const OSASCRIPT: &str = "/usr/bin/osascript";

/// The AppleScript that runs `program args…` as root. Every piece goes
/// through `quoted form of`, so no path can break out of the shell command.
pub fn applescript(program: &Path, args: &[&OsStr]) -> Result<String, String> {
    let mut parts = vec![program.as_os_str()];
    parts.extend_from_slice(args);
    let quoted = parts
        .into_iter()
        .map(|p| {
            let s = p
                .to_str()
                .ok_or_else(|| format!("{p:?} is not valid UTF-8"))?;
            if s.chars().any(char::is_control) {
                return Err(format!("{s:?} contains a control character"));
            }
            Ok(format!("quoted form of {}", applescript_string(s)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "do shell script {} with administrator privileges",
        quoted.join(" & \" \" & ")
    ))
}

/// An AppleScript string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// osascript reports a dismissed dialog as error -128 ("User canceled").
pub fn classify_osascript(success: bool, stderr: &str) -> Outcome {
    if success {
        Outcome::Exited(0)
    } else if stderr.contains("(-128)") {
        Outcome::Cancelled
    } else {
        Outcome::Failed(non_empty(stderr, "osascript failed"))
    }
}

// ---------------------------------------------------------------- Windows

/// Quote arguments for `CommandLineToArgvW`, the parser every Rust and C
/// program on Windows uses: backslashes are literal except before a quote.
pub fn windows_command_line(args: &[&str]) -> String {
    args.iter()
        .map(|a| {
            if !a.is_empty() && !a.contains([' ', '\t', '\n', '\u{b}', '"']) {
                return a.to_string();
            }
            let mut out = String::from('"');
            let mut backslashes = 0;
            for c in a.chars() {
                match c {
                    '\\' => backslashes += 1,
                    '"' => {
                        out.extend(std::iter::repeat('\\').take(backslashes * 2 + 1));
                        out.push('"');
                        backslashes = 0;
                    }
                    _ => {
                        out.extend(std::iter::repeat('\\').take(backslashes));
                        out.push(c);
                        backslashes = 0;
                    }
                }
            }
            out.extend(std::iter::repeat('\\').take(backslashes * 2));
            out.push('"');
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_line(s: &str) -> &str {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

fn non_empty(s: &str, fallback: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

// ------------------------------------------------------------ hand-off dir

/// A fresh directory only this user can enter, for the elevated program to
/// leave its result in. Removed, with its contents, when dropped.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// Create `<temp>/<prefix><unique>`. Creation fails rather than reuse an
    /// existing path, so nobody can pre-plant the directory or a link to it.
    pub fn create(prefix: &str) -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for attempt in 0..16u32 {
            let path = base.join(format!(
                "{prefix}{}-{:x}-{attempt}",
                std::process::id(),
                seed
            ));
            #[cfg_attr(not(unix), allow(unused_mut))]
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            match builder.create(&path) {
                Ok(()) => return Ok(PrivateDir { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not create a unique private directory",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Whether `path` has the exact shape of a hand-off file: absolute, no `..`,
/// named `file`, directly inside a directory whose name starts with `prefix`.
/// The elevated side checks this before writing anything, so its write
/// access cannot be pointed at an arbitrary location through its arguments.
pub fn is_handoff_path(path: &Path, prefix: &str, file: &str) -> bool {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    if path.file_name() != Some(OsStr::new(file)) {
        return false;
    }
    path.parent()
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
        .is_some_and(|d| d.starts_with(prefix) && d.len() > prefix.len())
}

// -------------------------------------------------------------- platforms

#[cfg(all(unix, not(target_os = "macos")))]
mod sys {
    use super::*;
    use std::process::Command;

    pub fn run(program: &Path, args: &[&OsStr]) -> Outcome {
        match Command::new("pkexec").arg(program).args(args).output() {
            Ok(out) => classify_pkexec(out.status.code(), &String::from_utf8_lossy(&out.stderr)),
            Err(e) => Outcome::Unavailable(format!("pkexec could not start: {e}")),
        }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use super::*;
    use std::process::Command;

    pub fn run(program: &Path, args: &[&OsStr]) -> Outcome {
        let script = match applescript(program, args) {
            Ok(s) => s,
            Err(e) => return Outcome::Failed(e),
        };
        match Command::new(OSASCRIPT).arg("-e").arg(script).output() {
            Ok(out) => {
                classify_osascript(out.status.success(), &String::from_utf8_lossy(&out.stderr))
            }
            Err(e) => Outcome::Unavailable(format!("osascript could not start: {e}")),
        }
    }
}

#[cfg(windows)]
mod sys {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, WaitForSingleObject, INFINITE,
    };
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn run(program: &Path, args: &[&OsStr]) -> Outcome {
        let Some(program) = program.to_str() else {
            return Outcome::Failed("program path is not valid Unicode".into());
        };
        let args = match args
            .iter()
            .map(|a| a.to_str().ok_or("an argument is not valid Unicode"))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(a) => a,
            Err(e) => return Outcome::Failed(e.into()),
        };
        let verb = wide("runas");
        let file = wide(program);
        let params = wide(&windows_command_line(&args));

        // SAFETY: SHELLEXECUTEINFOW is plain data; every pointer stays alive
        // for the call, and cbSize is set as the API requires.
        let mut info: SHELLEXECUTEINFOW = unsafe { zeroed() };
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = 0; // SW_HIDE: the elevated copy opens no window

        // SAFETY: info is fully initialised above.
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            // SAFETY: reads the calling thread's last error.
            let err = unsafe { GetLastError() };
            return if err == ERROR_CANCELLED {
                Outcome::Cancelled
            } else {
                Outcome::Failed(std::io::Error::from_raw_os_error(err as i32).to_string())
            };
        }
        if info.hProcess.is_null() {
            return Outcome::Failed("the elevated process handle was not returned".into());
        }
        let mut code = 0u32;
        // SAFETY: hProcess is a live handle we own and close exactly once.
        let ok = unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let ok = GetExitCodeProcess(info.hProcess, &mut code);
            CloseHandle(info.hProcess);
            ok
        };
        if ok == 0 {
            return Outcome::Failed("could not read the elevated process's exit status".into());
        }
        Outcome::Exited(code as i32)
    }
}

#[cfg(not(any(windows, unix)))]
mod sys {
    use super::*;

    pub fn run(_: &Path, _: &[&OsStr]) -> Outcome {
        Outcome::Unavailable("this operating system has no supported elevation prompt".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applescript_quotes_every_piece() {
        let s = applescript(
            Path::new("/Applications/probe.app/Contents/MacOS/probe"),
            &[
                OsStr::new("--elevated-scan"),
                OsStr::new("/tmp/it's \"x\"\\y/scan.json"),
            ],
        )
        .unwrap();
        assert_eq!(
            s,
            "do shell script quoted form of \"/Applications/probe.app/Contents/MacOS/probe\" & \" \" & \
             quoted form of \"--elevated-scan\" & \" \" & \
             quoted form of \"/tmp/it's \\\"x\\\"\\\\y/scan.json\" with administrator privileges"
        );
        assert!(applescript(Path::new("/a"), &[OsStr::new("x\ny")]).is_err());
    }

    #[test]
    fn windows_arguments_survive_command_line_parsing() {
        assert_eq!(
            windows_command_line(&["--elevated-scan"]),
            "--elevated-scan"
        );
        assert_eq!(
            windows_command_line(&[
                "--elevated-scan",
                r"C:\Users\Ada Lovelace\AppData\Local\Temp\p-1\scan.json"
            ]),
            r#"--elevated-scan "C:\Users\Ada Lovelace\AppData\Local\Temp\p-1\scan.json""#
        );
        assert_eq!(
            windows_command_line(&[r"C:\dir with space\"]),
            r#""C:\dir with space\\""#
        );
        assert_eq!(windows_command_line(&[r#"say "hi""#]), r#""say \"hi\"""#);
        assert_eq!(windows_command_line(&[""]), r#""""#);
    }

    #[test]
    fn classifies_prompt_outcomes() {
        assert_eq!(classify_pkexec(Some(0), ""), Outcome::Exited(0));
        assert_eq!(classify_pkexec(Some(3), ""), Outcome::Exited(3));
        assert_eq!(classify_pkexec(Some(126), ""), Outcome::Cancelled);
        assert_eq!(
            classify_pkexec(
                Some(127),
                "Error executing command as another user: Not authorized\n\nThis incident has been reported.\n"
            ),
            Outcome::Unavailable("Error executing command as another user: Not authorized".into())
        );
        assert_eq!(
            classify_pkexec(Some(127), ""),
            Outcome::Unavailable("not authorised".into())
        );
        assert!(matches!(classify_pkexec(None, ""), Outcome::Failed(_)));

        assert_eq!(classify_osascript(true, ""), Outcome::Exited(0));
        assert_eq!(
            classify_osascript(false, "0:93: execution error: User canceled. (-128)"),
            Outcome::Cancelled
        );
        assert!(matches!(
            classify_osascript(false, "execution error: boom (1)"),
            Outcome::Failed(_)
        ));
    }

    #[test]
    fn linux_says_why_it_cannot_prompt() {
        assert!(linux_available(false, true).is_ok());
        assert!(linux_available(true, true)
            .unwrap_err()
            .contains("AppImage"));
        assert!(linux_available(false, false)
            .unwrap_err()
            .contains("pkexec"));
    }

    #[test]
    fn handoff_paths_have_one_shape() {
        let base = std::env::temp_dir();
        let ok = base.join("probe-elevated-123-abc-0").join("scan.json");
        assert!(is_handoff_path(&ok, "probe-elevated-", "scan.json"));
        for bad in [
            PathBuf::from("probe-elevated-1/scan.json"),
            base.join("probe-elevated-1").join("other.json"),
            base.join("somewhere").join("scan.json"),
            base.join("probe-elevated-").join("scan.json"),
            base.join("probe-elevated-1").join("..").join("scan.json"),
            base.join("probe-elevated-1").join("x").join("scan.json"),
        ] {
            assert!(
                !is_handoff_path(&bad, "probe-elevated-", "scan.json"),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn private_dirs_are_unique_owner_only_and_cleaned_up() {
        let a = PrivateDir::create("elevate-test-").unwrap();
        let b = PrivateDir::create("elevate-test-").unwrap();
        assert_ne!(a.path(), b.path());
        assert!(a.path().is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(a.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        std::fs::write(a.path().join("scan.json"), "{}").unwrap();
        let path = a.path().to_path_buf();
        drop(a);
        assert!(!path.exists());
    }
}
