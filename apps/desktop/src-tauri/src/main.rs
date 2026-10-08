// No console window behind the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The elevated copy of the app only scans and exits. It is handled
    // before anything else runs, so it never creates a window or web view.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(code) = probe_desktop_lib::privilege::handoff_entry(&args) {
        std::process::exit(code);
    }
    probe_desktop_lib::run()
}
