//! hwprobe: hardware extraction and condition grading for used-laptop verification.
//!
//! The crate is independent of any GUI. It compiles into the desktop shell,
//! the technician CLI and the CI test harness alike.
//!
//! ```no_run
//! let scan = hwprobe::scan();
//! let grade = hwprobe::grade::grade(&scan);
//! println!("{}", hwprobe::report::render(&scan, &grade));
//! ```

pub mod backend;
pub mod fingerprint;
pub mod grade;
pub mod model;
pub mod parse;
pub mod report;

pub use backend::{is_elevated, scan};
pub use fingerprint::Fingerprint;
pub use grade::{Grade, Verdict};
pub use model::Scan;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
