//! Hands-on tests: checks a person runs with the app's guidance, such as
//! pressing every key, sweeping the touchpad, looking at full-screen colours
//! and listening to each speaker.
//!
//! Unlike a [`Scan`](crate::Scan), these are observations, not hardware
//! reads: the app records which keys produced events and what the tester
//! reported seeing or hearing. They are kept beside a scan, never inside it,
//! so a scan's content id does not change when tests are run later.

use serde::{Deserialize, Serialize};

/// Bumped when the shape of [`HandsOn`] changes incompatibly.
pub const HANDS_ON_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    Keyboard,
    Pointer,
    Screen,
    Speakers,
    Physical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Everything the test covers worked.
    Passed,
    /// The test found at least one problem, listed in `problems`.
    Problems,
    /// Started but not finished: neither a pass nor a fault.
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub kind: TestKind,
    pub outcome: Outcome,
    /// Unix seconds when the test finished.
    pub tested_at: u64,
    /// One plain sentence, e.g. "All 78 keys registered".
    pub summary: String,
    /// One line per problem, e.g. "Key K did not register".
    #[serde(default)]
    pub problems: Vec<String>,
    /// Test-specific measurements (keys seen, refresh rate, pointer area).
    #[serde(default)]
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HandsOn {
    pub version: u32,
    /// The latest result of each kind of test; a re-run replaces the old one.
    pub tests: Vec<TestResult>,
}

impl HandsOn {
    pub fn result(&self, kind: TestKind) -> Option<&TestResult> {
        self.tests.iter().find(|t| t.kind == kind)
    }

    /// Record a result, replacing any earlier result of the same kind.
    pub fn record(&mut self, result: TestResult) {
        self.version = HANDS_ON_VERSION;
        self.tests.retain(|t| t.kind != result.kind);
        self.tests.push(result);
        self.tests.sort_by_key(|t| t.kind);
    }

    /// Results that found problems.
    pub fn failed(&self) -> impl Iterator<Item = &TestResult> {
        self.tests.iter().filter(|t| t.outcome == Outcome::Problems)
    }
}

pub fn kind_label(kind: TestKind) -> &'static str {
    match kind {
        TestKind::Keyboard => "Keyboard",
        TestKind::Pointer => "Touchpad and mouse",
        TestKind::Screen => "Screen",
        TestKind::Speakers => "Speakers",
        TestKind::Physical => "Physical inspection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(kind: TestKind, outcome: Outcome) -> TestResult {
        TestResult {
            kind,
            outcome,
            tested_at: 1,
            summary: String::new(),
            problems: Vec::new(),
            detail: serde_json::Value::Null,
        }
    }

    #[test]
    fn a_rerun_replaces_the_earlier_result() {
        let mut h = HandsOn::default();
        h.record(result(TestKind::Screen, Outcome::Problems));
        h.record(result(TestKind::Keyboard, Outcome::Passed));
        h.record(result(TestKind::Screen, Outcome::Passed));
        assert_eq!(h.tests.len(), 2);
        assert_eq!(
            h.tests[0].kind,
            TestKind::Keyboard,
            "kept in a stable order"
        );
        assert_eq!(h.failed().count(), 0);
        assert_eq!(h.version, HANDS_ON_VERSION);
    }

    #[test]
    fn reads_results_without_optional_fields() {
        let h: HandsOn = serde_json::from_str(
            r#"{"version":1,"tests":[{"kind":"speakers","outcome":"passed","tested_at":5,"summary":"ok"}]}"#,
        )
        .unwrap();
        assert!(h.result(TestKind::Speakers).unwrap().problems.is_empty());
    }
}
