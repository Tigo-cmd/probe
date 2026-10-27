//! Seller mode: a short summary of a scan that travels as a link and a QR
//! code, so a buyer can see it on their phone and then check it against
//! the computer in front of them.
//!
//! The summary is not signed: anyone could edit the link. Its job is to
//! start the conversation and get the buyer to run probe themselves, which
//! [`compare`] then uses to confirm or contradict it. Identifiers travel
//! only as short one-way hashes, so the link does not publish serial
//! numbers; the system serial appears masked, for comparing with the
//! sticker.

use serde::{Deserialize, Serialize};

use crate::assess::Assessment;
use crate::fingerprint::Fingerprint;
use crate::grade::{Grade, Verdict};
use crate::handson::{HandsOn, Outcome};
use crate::model::{DriveHealth, Scan};

/// Where the buyer's phone opens the summary. The summary rides in the
/// part after `#`, which browsers never send to the server.
pub const CHECK_URL: &str = "https://probe-pc.github.io/check";

pub const SUMMARY_VERSION: u8 = 1;

/// Field names are one or two letters to keep the QR code small.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Format version.
    pub v: u8,
    /// Scan time, Unix seconds.
    pub t: u64,
    /// Maker and model.
    pub m: String,
    /// System serial, masked, e.g. "PF••••CDE".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<String>,
    pub os: String,
    /// Condition score 0–100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<u8>,
    /// Overall verdict: "green", "amber", "red", "not_graded".
    pub h: String,
    /// Area scores: drives, battery, ownership.
    pub a: [Option<u8>; 3],
    /// Battery capacity against design, percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<u8>,
    /// Drive life left (worst internal NVMe drive), percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<u8>,
    /// Processor, e.g. "Intel Core i5, 8th gen, low-power".
    pub p: String,
    /// Memory in GB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<u32>,
    /// Main drive size in GB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub g: Option<u32>,
    /// Windows 11 readiness.
    pub w: String,
    /// Hands-on tests: passed, finished.
    pub x: [u8; 2],
    /// Problems found by hands-on tests, a few words each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub xp: Vec<String>,
    /// Whether any lock or management was found.
    pub l: bool,
    /// Hashed identifiers (6 hex digits each).
    pub i: Vec<String>,
}

impl Summary {
    pub fn of(scan: &Scan, grade: &Grade, a: &Assessment, hands_on: Option<&HandsOn>) -> Self {
        let id = &scan.identity;
        let vendor = id.vendor.value.as_deref().unwrap_or("").trim();
        let model = id.model.value.as_deref().unwrap_or("").trim();
        let name = if model.to_lowercase().starts_with(&vendor.to_lowercase()) {
            model.to_string()
        } else {
            format!("{vendor} {model}").trim().to_string()
        };
        let drive_life = scan
            .storage
            .iter()
            .filter(|d| !d.removable)
            .filter_map(|d| match &d.health {
                DriveHealth::Nvme(h) => Some(100u8.saturating_sub(h.percentage_used.min(100))),
                _ => None,
            })
            .min();
        let (passed, finished, problems) = match hands_on {
            Some(h) => {
                let done: Vec<_> = h
                    .tests
                    .iter()
                    .filter(|t| t.outcome != Outcome::Incomplete)
                    .collect();
                let passed = done.iter().filter(|t| t.outcome == Outcome::Passed).count();
                let problems = done
                    .iter()
                    .filter(|t| t.outcome == Outcome::Problems)
                    .map(|t| format!("{}: {}", crate::handson::kind_label(t.kind), t.summary))
                    .take(4)
                    .collect();
                (passed as u8, done.len() as u8, problems)
            }
            None => (0, 0, Vec::new()),
        };
        let area = |n: usize| a.condition.areas.get(n).and_then(|x| x.score);
        Summary {
            v: SUMMARY_VERSION,
            t: scan.started_at,
            m: if name.is_empty() {
                "Unidentified computer".into()
            } else {
                name
            },
            s: id.serial.value.as_deref().map(mask),
            os: scan.os.clone(),
            c: a.condition.score,
            h: verdict_name(grade.headline).into(),
            a: [area(0), area(1), area(2)],
            b: a.specs.battery_percent,
            d: drive_life,
            p: a.specs.cpu.summary.clone(),
            r: a.specs.ram_gb,
            g: a.specs
                .storage
                .capacity_bytes
                .map(|b| (b / 1_000_000_000) as u32),
            w: serde_json::to_value(a.windows11.readiness)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            x: [passed, finished],
            xp: problems,
            l: scan.encumbrance.iter().any(|s| s.present),
            i: hashes(&Fingerprint::of(scan)),
        }
    }

    /// The link a buyer opens: the check page with the summary after `#`.
    pub fn link(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        format!("{CHECK_URL}#{}", base64url(&json))
    }

    /// Read a summary from a link (or just the part after `#`).
    pub fn from_link(link: &str) -> Result<Summary, String> {
        let code = link.rsplit('#').next().unwrap_or(link).trim();
        let bytes = unbase64url(code).ok_or("This is not a probe seller link")?;
        let s: Summary =
            serde_json::from_slice(&bytes).map_err(|_| "This is not a probe seller link")?;
        if s.v > SUMMARY_VERSION {
            return Err(
                "This seller report was made by a newer probe: update probe to read it".into(),
            );
        }
        Ok(s)
    }

    /// The link as an SVG QR code, `size` pixels square.
    pub fn qr_svg(&self, size: u32) -> String {
        match qrcode::QrCode::with_error_correction_level(self.link(), qrcode::EcLevel::L) {
            Ok(code) => code
                .render::<qrcode::render::svg::Color>()
                .min_dimensions(size, size)
                .quiet_zone(true)
                .build(),
            Err(_) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    /// Identifiers in the seller's report match this computer.
    Same,
    /// Both have identifiers and none match: a different computer.
    Different,
    /// Not enough identifiers on one side to tell.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub identity: Match,
    pub matched: usize,
    pub compared: usize,
    pub seller: Summary,
    /// Where the seller's report and this scan disagree, in plain words.
    pub differences: Vec<String>,
}

/// Compare a seller's summary with a scan of the computer in front of the buyer.
pub fn compare(seller: Summary, scan: &Scan, grade: &Grade, a: &Assessment) -> Check {
    let mine = Summary::of(scan, grade, a, None);
    let matched = seller.i.iter().filter(|h| mine.i.contains(h)).count();
    let compared = seller.i.len().min(mine.i.len());
    let identity = if matched > 0 && matched * 2 >= compared {
        Match::Same
    } else if compared > 0 && matched == 0 {
        Match::Different
    } else {
        Match::Unknown
    };
    let mut differences = Vec::new();
    if seller.m != mine.m {
        differences.push(format!(
            "Model: the report says {}, this computer reports {}",
            seller.m, mine.m
        ));
    }
    if let (Some(s), Some(m)) = (&seller.s, &mine.s) {
        if s != m {
            differences.push(format!(
                "Serial number: the report shows {s}, this computer {m}"
            ));
        }
    }
    let worse =
        |what: &str, then: Option<u8>, now: Option<u8>, slack: u8, out: &mut Vec<String>| {
            if let (Some(t), Some(n)) = (then, now) {
                if n + slack < t {
                    out.push(format!("{what}: {t} in the report, {n} now"));
                }
            }
        };
    worse("Condition score", seller.c, mine.c, 5, &mut differences);
    worse(
        "Battery capacity (%)",
        seller.b,
        mine.b,
        3,
        &mut differences,
    );
    worse("Drive life left (%)", seller.d, mine.d, 2, &mut differences);
    if mine.l && !seller.l {
        differences.push(
            "A lock or management setting is present now that the report did not show".into(),
        );
    }
    if seller.r.is_some() && mine.r.is_some() && seller.r != mine.r {
        differences.push(format!(
            "Memory: {} GB in the report, {} GB now",
            seller.r.unwrap_or(0),
            mine.r.unwrap_or(0)
        ));
    }
    if seller.g.is_some() && mine.g.is_some() && seller.g != mine.g {
        differences.push(format!(
            "Storage: {} GB in the report, {} GB now",
            seller.g.unwrap_or(0),
            mine.g.unwrap_or(0)
        ));
    }
    Check {
        identity,
        matched,
        compared,
        seller,
        differences,
    }
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Green => "green",
        Verdict::Amber => "amber",
        Verdict::Red => "red",
        Verdict::NotGraded => "not_graded",
    }
}

/// Keep the start and end of a serial so it can be matched with the sticker.
pub(crate) fn mask(serial: &str) -> String {
    let c: Vec<char> = serial.trim().chars().collect();
    match c.len() {
        0 => String::new(),
        1..=4 => "•".repeat(c.len()),
        5..=7 => format!(
            "{}{}",
            "•".repeat(c.len() - 3),
            c[c.len() - 3..].iter().collect::<String>()
        ),
        n => format!(
            "{}{}{}",
            c[..2].iter().collect::<String>(),
            "•".repeat(n - 5),
            c[n - 3..].iter().collect::<String>()
        ),
    }
}

/// Short one-way hashes of each identifier, sorted.
fn hashes(f: &Fingerprint) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut add = |kind: &str, v: &str| {
        let v = v.trim().to_ascii_lowercase();
        if !v.is_empty() {
            ids.push(format!(
                "{:06x}",
                fnv1a(format!("{kind}:{v}").as_bytes()) >> 40
            ));
        }
    };
    if let Some(u) = &f.smbios_uuid {
        add("uuid", u);
    }
    if let Some(s) = &f.board_serial {
        add("board", s);
    }
    if let Some(s) = &f.system_serial {
        add("system", s);
    }
    for s in &f.drive_serials {
        add("drive", s);
    }
    for s in &f.battery_serials {
        add("battery", s);
    }
    for m in &f.macs {
        add("mac", m);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 4 / 3 + 3);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn unbase64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = B64.iter().position(|&x| x == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn scan(serial: &str, drive_serial: &str) -> Scan {
        Scan {
            os: "windows".into(),
            started_at: 1_793_000_000,
            identity: crate::model::Identity {
                vendor: Field::claimed(Some("LENOVO".into()), "t"),
                model: Field::claimed(Some("20HRCTO1WW".into()), "t"),
                serial: Field::claimed(Some(serial.into()), "t"),
                uuid: Field::claimed(Some("4C4C4544-0042".into()), "t"),
                ..crate::model::Identity::default()
            },
            storage: vec![Drive {
                name: "disk0".into(),
                transport: Transport::Nvme,
                rotational: Field::default(),
                removable: false,
                model: Field::default(),
                serial: Field::claimed(Some(drive_serial.into()), "t"),
                firmware: Field::default(),
                capacity_bytes: Field::claimed(Some(512_000_000_000), "t"),
                health: DriveHealth::Unavailable { reason: "t".into() },
            }],
            ..Scan::default()
        }
    }

    fn summary(s: &Scan) -> (Summary, Grade, Assessment) {
        let g = crate::grade::grade(s);
        let a = crate::assess::assess(s, &g);
        (Summary::of(s, &g, &a, None), g, a)
    }

    #[test]
    fn base64url_round_trips() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"abcd", b"\xff\x00\x10probe"] {
            assert_eq!(unbase64url(&base64url(data)).unwrap(), data);
        }
    }

    #[test]
    fn link_round_trips_and_hides_serials() {
        let s = scan("PF0ABCDE", "S4EWNX0R123456");
        let (sum, ..) = summary(&s);
        let link = sum.link();
        assert!(link.starts_with("https://probe-pc.github.io/check#"));
        let back = Summary::from_link(&link).unwrap();
        assert_eq!(back, sum);
        assert_eq!(back.s.as_deref(), Some("PF•••CDE"));
        let json =
            String::from_utf8(unbase64url(link.split('#').nth(1).unwrap()).unwrap()).unwrap();
        assert!(!json.contains("S4EWNX0R123456") && !json.contains("PF0ABCDE"));
        assert!(link.len() < 900, "keep the QR code small: {}", link.len());
        assert!(sum.qr_svg(200).starts_with("<?xml"));
    }

    #[test]
    fn same_computer_matches_and_a_swap_does_not() {
        let original = scan("PF0ABCDE", "DRIVE1");
        let (seller, ..) = summary(&original);

        let (_, g, a) = summary(&original);
        let same = compare(seller.clone(), &original, &g, &a);
        assert_eq!(same.identity, Match::Same);
        assert!(same.differences.is_empty());

        let other = scan("ZZ9XYZ12", "DRIVE9");
        let mut other = other;
        other.identity.uuid = Field::claimed(Some("ffff".into()), "t");
        let (_, g, a) = summary(&other);
        let diff = compare(seller, &other, &g, &a);
        assert_eq!(diff.identity, Match::Different);
        assert!(diff
            .differences
            .iter()
            .any(|d| d.starts_with("Serial number")));
    }

    #[test]
    fn rejects_other_links() {
        assert!(Summary::from_link("https://example.com/#hello").is_err());
        assert!(Summary::from_link("").is_err());
    }
}
