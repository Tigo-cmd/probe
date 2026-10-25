//! Condition score and fit for purpose: the two questions a buyer asks that
//! the grader deliberately leaves alone. "How good is it, as one number?" and
//! "is it right for what I will do with it?"
//!
//! The condition score never contradicts the grade. It is capped by the
//! headline verdict, so a machine with a red finding cannot score above 39
//! however good its other areas are. Areas without evidence are left out
//! and named, never counted as full marks.
//!
//! Fit for purpose weighs the specification against each kind of use, then
//! applies the condition: a failing drive or a management lock limits every
//! use. Processor and graphics tiers are estimates from model names, which
//! are claims, so every reason drawn from them is tagged inferred. The
//! thresholds are this app's own conventions, versioned as [`ASSESS_VERSION`].

use serde::{Deserialize, Serialize};

use crate::grade::{Axis, Grade, Verdict};
use crate::handson::{self, HandsOn, Outcome, TestKind};
use crate::model::*;

/// Bump whenever a weight, threshold or tier rule changes.
pub const ASSESS_VERSION: &str = "1.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub version: String,
    pub condition: Condition,
    pub specs: Specs,
    /// One entry per [`UseCase`], in [`USE_CASES`] order.
    pub uses: Vec<UseFit>,
    /// Uses rated good or great, best first.
    pub best_for: Vec<UseCase>,
    /// Practical next steps, most important first.
    pub tips: Vec<String>,
    /// What the hands-on tests found, when any were run.
    pub hands_on: Option<HandsOnSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandsOnSummary {
    /// Tests with a finished result (passed or problems).
    pub finished: usize,
    pub passed: usize,
    /// "Keyboard: K did not register", one line per problem.
    pub problems: Vec<String>,
    /// Tests not run yet, so the report can suggest them.
    pub not_run: Vec<TestKind>,
}

const ALL_TESTS: [TestKind; 5] = [
    TestKind::Keyboard,
    TestKind::Pointer,
    TestKind::Screen,
    TestKind::Speakers,
    TestKind::Physical,
];

// --------------------------------------------------------------- condition

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    Excellent,
    Good,
    Fair,
    Poor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    /// 0–100. `None` when no area had evidence.
    pub score: Option<u8>,
    pub band: Option<Band>,
    /// The ceiling the headline verdict put on the score, when it applied.
    pub cap: Option<u8>,
    /// Areas that produced a score, out of all areas.
    pub scored_areas: usize,
    pub areas: Vec<AreaScore>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreaScore {
    pub axis: Axis,
    pub verdict: Verdict,
    /// 0–100, `None` when the area had no evidence.
    pub score: Option<u8>,
    /// Share of the overall score, in percent, when scored.
    pub weight: u8,
    /// One plain sentence: what the score is made of, or why there is none.
    pub summary: String,
}

const AREA_WEIGHTS: [(Axis, u32); 3] = [
    (Axis::Function, 45),
    (Axis::Battery, 25),
    (Axis::Encumbrance, 30),
];

fn condition(scan: &Scan, grade: &Grade) -> Condition {
    let areas: Vec<AreaScore> = AREA_WEIGHTS
        .iter()
        .map(|&(axis, weight)| {
            let verdict = grade
                .axes
                .iter()
                .find(|a| a.axis == axis)
                .map(|a| a.verdict)
                .unwrap_or(Verdict::NotGraded);
            let (score, summary) = match axis {
                Axis::Function => drives_area(scan, grade),
                Axis::Battery => battery_area(scan),
                Axis::Encumbrance => encumbrance_area(scan, verdict),
            };
            AreaScore {
                axis,
                verdict,
                score,
                weight: weight as u8,
                summary,
            }
        })
        .collect();

    let scored: Vec<(u32, u32)> = areas
        .iter()
        .filter_map(|a| a.score.map(|s| (a.weight as u32, s as u32)))
        .collect();
    let total: u32 = scored.iter().map(|(w, _)| w).sum();
    let raw = (total > 0).then(|| {
        let sum: u32 = scored.iter().map(|(w, s)| w * s).sum();
        ((sum as f64 / total as f64).round() as u32).min(100) as u8
    });
    let ceiling = match grade.headline {
        Verdict::Red => Some(39),
        Verdict::Amber => Some(79),
        _ => None,
    };
    let cap = raw.zip(ceiling).filter(|(r, c)| r > c).map(|(_, c)| c);
    let score = raw.map(|r| cap.map_or(r, |c| r.min(c)));
    Condition {
        score,
        band: score.map(|s| match s {
            90.. => Band::Excellent,
            75..=89 => Band::Good,
            55..=74 => Band::Fair,
            _ => Band::Poor,
        }),
        cap,
        scored_areas: scored.len(),
        areas,
    }
}

fn internal(drive: &Drive) -> bool {
    drive.transport != Transport::Usb && !drive.removable
}

/// The worst internal drive sets the score: wear left, held down by any
/// red or amber finding on that drive.
fn drives_area(scan: &Scan, grade: &Grade) -> (Option<u8>, String) {
    let function = grade.axes.iter().find(|a| a.axis == Axis::Function);
    let mut worst: Option<(u8, String)> = None;
    for d in scan.storage.iter().filter(|d| internal(d)) {
        let component = format!("storage/{}", d.name);
        let verdict = function
            .map(|a| {
                a.findings
                    .iter()
                    .filter(|f| f.component == component)
                    .map(|f| f.verdict)
                    .max()
                    .unwrap_or(Verdict::NotGraded)
            })
            .unwrap_or(Verdict::NotGraded);
        if verdict == Verdict::NotGraded {
            continue;
        }
        let (base, what) = match &d.health {
            DriveHealth::Nvme(h) => {
                let left = 100u8.saturating_sub(h.percentage_used.min(100));
                (left, format!("about {left}% of rated life left"))
            }
            _ => (100, "no wear figure reported".to_string()),
        };
        let (score, state) = match verdict {
            Verdict::Red => (base.min(15), "Failure signs"),
            Verdict::Amber => (base.min(60), "Needs attention"),
            _ => (base, "No faults detected"),
        };
        let line = format!("{state} on {}, {what}", d.name);
        if worst.as_ref().map_or(true, |(s, _)| score < *s) {
            worst = Some((score, line));
        }
    }
    match worst {
        Some((s, line)) => (Some(s), line),
        None => (
            None,
            "Not scored: no internal drive's health could be read".into(),
        ),
    }
}

/// Full-charge capacity as a share of design, when both are comparable.
fn state_of_health(b: &Battery) -> Option<f64> {
    let (d, f) = (b.design_capacity.get()?, b.full_charge_capacity.get()?);
    // Exactly equal is a firmware default, not a perfect pack (see grade.rs).
    (d.unit == f.unit && d.value > 0 && f.value != d.value)
        .then(|| f.value as f64 * 100.0 / d.value as f64)
}

fn battery_area(scan: &Scan) -> (Option<u8>, String) {
    let mut worst: Option<(u8, String)> = None;
    for b in &scan.batteries {
        let Some(soh) = state_of_health(b) else {
            continue;
        };
        let mut score = if soh > 105.0 {
            60
        } else {
            soh.min(100.0).round() as u8
        };
        let mut line = if soh > 105.0 {
            format!("Battery reports {soh:.0}% of design, which is not physically expected")
        } else {
            format!("Battery holds {soh:.0}% of its original capacity")
        };
        if let Some(&c) = b.cycle_count.get() {
            if c > 1000 {
                score = score.min(70);
            }
            line.push_str(&format!(", {c} charge cycles"));
        }
        if worst.as_ref().map_or(true, |(s, _)| score < *s) {
            worst = Some((score, line));
        }
    }
    match worst {
        Some((s, line)) => (Some(s), line),
        None if scan.batteries.is_empty() => (None, "Not scored: no battery was detected".into()),
        None => (
            None,
            "Not scored: the battery did not report its capacity".into(),
        ),
    }
}

fn encumbrance_area(scan: &Scan, verdict: Verdict) -> (Option<u8>, String) {
    let present: Vec<&str> = scan
        .encumbrance
        .iter()
        .filter(|s| s.present)
        .map(|s| s.detail.as_str())
        .collect();
    match verdict {
        Verdict::Green => (
            Some(100),
            format!(
                "No locks or management found in {} checks",
                scan.encumbrance.len()
            ),
        ),
        Verdict::Amber => (Some(60), sentence_case(&present.join("; "))),
        Verdict::Red => (
            Some(0),
            format!(
                "Someone else may still control this laptop: {}",
                present.join("; ")
            ),
        ),
        Verdict::NotGraded if scan.os == "linux" => (
            None,
            "Not scored: management locks cannot be read from Linux".into(),
        ),
        Verdict::NotGraded => (None, "Not scored: the lock checks did not all run".into()),
    }
}

fn sentence_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

// ------------------------------------------------------------------- specs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Specs {
    pub cpu: CpuEstimate,
    /// Installed memory estimated from usable memory (`15.6 GiB` → 16).
    pub ram_gb: Option<u32>,
    pub storage: StorageSummary,
    pub gpu: GpuEstimate,
    pub display: Option<Resolution>,
    /// Worst battery's capacity against design, in percent.
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuEstimate {
    /// 1 basic, 2 entry, 3 mainstream, 4 strong, 5 high-end.
    pub tier: Option<u8>,
    /// e.g. "Intel Core i7, 7th gen, low-power".
    pub summary: String,
    /// An Arm processor: some Windows apps and most games do not run natively.
    pub arm: bool,
    pub apple: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Nvme,
    SataSsd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageSummary {
    pub kind: Option<StorageKind>,
    pub capacity_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuEstimate {
    /// 1 basic, 2 entry, 3 mainstream, 4 strong, 5 high-end.
    pub tier: Option<u8>,
    pub name: Option<String>,
    /// A separate graphics chip with its own memory.
    pub discrete: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u16,
    pub height: u16,
}

fn specs(scan: &Scan) -> Specs {
    let cpu = cpu_estimate(
        scan.cpu.brand.get().map(String::as_str),
        scan.cpu.physical_cores.get().copied(),
    );
    Specs {
        gpu: gpu_estimate(&scan.gpus, &cpu),
        cpu,
        ram_gb: scan.memory.total_bytes.get().map(|&b| installed_gb(b)),
        storage: storage_summary(scan),
        display: panel(&scan.displays),
        battery_percent: scan
            .batteries
            .iter()
            .filter_map(state_of_health)
            .map(|s| s.min(100.0).round() as u8)
            .min(),
    }
}

/// Usable memory is installed memory minus firmware and graphics
/// reservations; round up to the next size modules come in.
fn installed_gb(usable_bytes: u64) -> u32 {
    let gib = usable_bytes as f64 / (1u64 << 30) as f64;
    [2, 3, 4, 6, 8, 12, 16, 20, 24, 32, 36, 48, 64, 96, 128]
        .into_iter()
        .find(|&s| gib <= s as f64)
        .unwrap_or_else(|| gib.ceil() as u32)
}

/// The main internal drive: the largest one that is not external.
fn storage_summary(scan: &Scan) -> StorageSummary {
    let main = scan
        .storage
        .iter()
        .filter(|d| internal(d))
        .max_by_key(|d| d.capacity_bytes.get().copied().unwrap_or(0));
    StorageSummary {
        kind: main.map(|d| match (d.transport, d.rotational.get()) {
            (Transport::Nvme, _) => StorageKind::Nvme,
            (_, Some(true)) => StorageKind::Hdd,
            (Transport::Sata, Some(false)) => StorageKind::SataSsd,
            _ => StorageKind::Unknown,
        }),
        capacity_bytes: main.and_then(|d| d.capacity_bytes.get().copied()),
    }
}

/// The built-in panel when it can be told apart, else the sharpest display.
fn panel(displays: &[Display]) -> Option<Resolution> {
    let res = |d: &Display| {
        Some(Resolution {
            width: d.native_width?,
            height: d.native_height?,
        })
    };
    let builtin = |d: &&Display| {
        let c = d.connector.to_ascii_lowercase();
        c.contains("edp") || c.contains("lvds") || c.contains("dsi")
    };
    displays.iter().find(builtin).and_then(res).or_else(|| {
        displays
            .iter()
            .filter_map(res)
            .max_by_key(|r| r.width as u32 * r.height as u32)
    })
}

/// Estimate a processor's tier from its brand string and core count.
pub fn cpu_estimate(brand: Option<&str>, physical_cores: Option<u32>) -> CpuEstimate {
    let unknown = |summary: &str| CpuEstimate {
        tier: physical_cores.map(|c| match c {
            0..=1 => 1,
            2..=3 => 2,
            4..=5 => 3,
            _ => 4,
        }),
        summary: summary.into(),
        arm: false,
        apple: false,
    };
    let Some(brand) = brand else {
        return unknown("Processor not reported");
    };
    let b = brand
        .to_ascii_lowercase()
        .replace("(r)", " ")
        .replace("(tm)", " ");
    let words: Vec<&str> = b
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .collect();
    let after = |key: &str| {
        words
            .iter()
            .position(|w| *w == key)
            .and_then(|i| words.get(i + 1))
            .copied()
    };
    let cores = physical_cores;
    // Never rate a chip above what its core count can deliver.
    let core_cap = |t: u8| match cores {
        Some(0..=1) => t.min(1),
        Some(2..=3) => t.min(2),
        Some(4..=5) => t.min(4),
        _ => t,
    };
    let est = |tier: u8, summary: String| CpuEstimate {
        tier: Some(core_cap(tier.clamp(1, 5))),
        summary,
        arm: false,
        apple: false,
    };

    // Apple silicon: "Apple M1", "Apple M2 Pro".
    if let Some(chip) = after("apple").filter(|c| c.starts_with('m')) {
        let gen: u8 = chip[1..].parse().unwrap_or(1);
        let variant = after(chip).filter(|v| ["pro", "max", "ultra"].contains(v));
        let tier = if variant.is_some() || gen >= 3 { 5 } else { 4 };
        let name = match variant {
            Some(v) => format!("Apple M{gen} {}{}", v[..1].to_uppercase(), &v[1..]),
            None => format!("Apple M{gen}"),
        };
        return CpuEstimate {
            tier: Some(tier),
            summary: name,
            arm: true,
            apple: true,
        };
    }
    if b.contains("snapdragon") {
        let tier = if b.contains("x elite") || b.contains("x plus") || b.contains(" x1") {
            4
        } else {
            2
        };
        return CpuEstimate {
            arm: true,
            ..est(tier, "Qualcomm Snapdragon (Arm)".into())
        };
    }
    for (needle, label) in [
        ("celeron", "Intel Celeron"),
        ("pentium", "Intel Pentium"),
        ("atom", "Intel Atom"),
        ("athlon", "AMD Athlon"),
    ] {
        if b.contains(needle) {
            let tier = if cores.unwrap_or(2) >= 4 { 2 } else { 1 };
            return est(tier, format!("{label}, a basic processor"));
        }
    }
    if b.contains("intel")
        && words
            .iter()
            .any(|w| ["n100", "n200", "n95", "n97", "n300", "n305"].contains(w))
    {
        return est(2, "Intel N-series, a basic processor".into());
    }
    if b.contains("amd")
        && words.iter().any(|w| {
            w.starts_with("a4")
                || w.starts_with("a6")
                || w.starts_with("a9")
                || w.starts_with("a10")
                || w.starts_with("a12")
                || w.starts_with("e1")
                || w.starts_with("e2")
        })
    {
        return est(1, "AMD A-series, a basic processor".into());
    }

    // Intel Core Ultra 5/7/9 and Core 3/5/7 (2023 on).
    if let Some(level) = after("ultra").and_then(|l| l.parse::<u8>().ok()) {
        let model = after(&level.to_string()).unwrap_or("");
        let h = model.ends_with('h') || model.ends_with("hx");
        let tier = if h || level >= 9 { 5 } else { 4 };
        let class = if h { "performance" } else { "low-power" };
        return est(tier, format!("Intel Core Ultra {level}, {class}"));
    }

    // Intel Core iN-XXXX[suffix].
    if let Some(i) = words
        .iter()
        .position(|w| ["i3", "i5", "i7", "i9"].contains(w))
    {
        let level: u8 = words[i][1..].parse().unwrap_or(5);
        let model = words.get(i + 1).copied().unwrap_or("");
        let digits: String = model.chars().take_while(|c| c.is_ascii_digit()).collect();
        let suffix = &model[digits.len()..];
        let gen: Option<u8> = match digits.len() {
            3 => Some(1),
            4 if digits.starts_with('1') => digits[..2].parse().ok(),
            4 => digits[..1].parse().ok(),
            5 => digits[..2].parse().ok(),
            _ => None,
        };
        let perf = suffix.contains('h');
        let low_power =
            suffix.starts_with('u') || suffix.starts_with('y') || suffix.starts_with('g');
        let mut tier: u8 = match gen {
            Some(0..=3) => {
                if perf || suffix.starts_with("qm") {
                    2
                } else {
                    1
                }
            }
            Some(4..=7) => {
                if perf || suffix.starts_with("qm") {
                    3
                } else {
                    2
                }
            }
            Some(8..=10) => {
                if perf {
                    4
                } else {
                    3
                }
            }
            Some(11) => {
                if perf {
                    5
                } else {
                    4
                }
            }
            Some(_) => {
                if perf {
                    5
                } else {
                    4
                }
            }
            None => 3,
        };
        if suffix.starts_with('y') || level == 3 {
            tier = tier.saturating_sub(1);
        }
        if level == 9 {
            tier += 1;
        }
        let mut summary = format!("Intel Core i{level}");
        if let Some(g) = gen {
            summary.push_str(&format!(", {}", ordinal_gen(g)));
        }
        if perf {
            summary.push_str(", performance");
        } else if low_power {
            summary.push_str(", low-power");
        }
        return est(tier, summary);
    }
    // Intel Core 3/5/7 NNN (Series 1/2, no "i").
    if let Some(level) = after("core").and_then(|l| l.parse::<u8>().ok()) {
        let model = after(&level.to_string()).unwrap_or("");
        let h = model.ends_with('h');
        return est(if h { 5 } else { 4 }, format!("Intel Core {level}"));
    }

    // AMD Ryzen N XXXX[suffix], Ryzen AI N HX NNN.
    if let Some(level) =
        after("ryzen").and_then(|l| l.parse::<u8>().ok().or_else(|| (l == "ai").then_some(0)))
    {
        if level == 0 {
            let tier = if b.contains(" hx ") || b.contains("ai 9") {
                5
            } else {
                4
            };
            return est(tier, "AMD Ryzen AI".into());
        }
        let model = words
            .iter()
            .skip_while(|w| **w != "ryzen")
            .skip(2)
            .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .copied()
            .unwrap_or("");
        let digits: String = model.chars().take_while(|c| c.is_ascii_digit()).collect();
        let suffix = &model[digits.len()..];
        let series: u8 = digits.get(..1).and_then(|d| d.parse().ok()).unwrap_or(0);
        let perf = suffix.starts_with('h');
        let tier: u8 = match series {
            // Zen and Zen+ (2000, 3000): four cores at most.
            0..=3 => {
                if perf || level >= 5 {
                    3
                } else {
                    2
                }
            }
            // 4000 and later: six to eight cores on Ryzen 5 and 7.
            _ => {
                let t = if perf || suffix.is_empty() { 5 } else { 4 };
                if level == 3 {
                    t - 1
                } else {
                    t
                }
            }
        };
        let mut summary = format!("AMD Ryzen {level}");
        if series > 0 {
            summary.push_str(&format!(", {series}000 series"));
        }
        if perf {
            summary.push_str(", performance");
        } else if suffix.starts_with('u') {
            summary.push_str(", low-power");
        }
        return est(tier, summary);
    }
    if b.contains("xeon") {
        return est(4, "Intel Xeon, workstation".into());
    }
    unknown(brand.trim())
}

fn ordinal_gen(g: u8) -> String {
    let suffix = match (g % 10, g % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{g}{suffix} gen")
}

/// The strongest graphics adapter, rated by name.
fn gpu_estimate(gpus: &[Gpu], cpu: &CpuEstimate) -> GpuEstimate {
    gpus.iter()
        .filter_map(|g| {
            let name = g.name.get()?;
            let (tier, discrete) = gpu_tier(name, cpu);
            Some(GpuEstimate {
                tier: Some(tier),
                name: Some(name.clone()),
                discrete: Some(discrete),
            })
        })
        .max_by_key(|g| (g.tier, g.discrete))
        .unwrap_or(GpuEstimate {
            tier: None,
            name: None,
            discrete: None,
        })
}

/// Tier and whether it is a separate chip, from the adapter's name.
pub fn gpu_tier(name: &str, cpu: &CpuEstimate) -> (u8, bool) {
    let n = name.to_ascii_lowercase();
    let number_after = |key: &str| -> Option<u32> {
        let rest = &n[n.find(key)? + key.len()..];
        let digits: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().ok()
    };

    if n.contains("apple") {
        let gen = n
            .split_whitespace()
            .find_map(|w| w.strip_prefix('m')?.parse::<u8>().ok())
            .unwrap_or(1);
        let tier = if n.contains("max") || n.contains("ultra") {
            5
        } else if n.contains("pro") || gen >= 3 {
            4
        } else {
            3
        };
        return (tier, false);
    }
    if let Some(m) = number_after("rtx a") {
        // Workstation RTX A1000 to A5500.
        return (if m >= 3000 { 4 } else { 3 }, true);
    }
    if let Some(m) = number_after("rtx") {
        // RTX 2060, 3050, 4070: the tens digit is the level within a series.
        let level = (m % 100) / 10;
        let tier = match m {
            2000..=2099 if level <= 6 => 4,
            2000..=2099 => 5,
            _ if level <= 5 => 4,
            _ => 5,
        };
        return (tier, true);
    }
    if let Some(m) = number_after("gtx") {
        let tier = match m {
            1000..=1059 | 1600..=1659 => 3,
            1060..=1099 | 1660..=1699 => 4,
            _ => 2,
        };
        return (tier, true);
    }
    if n.contains("geforce mx") || (n.contains(" mx") && n.contains("nvidia")) {
        return (2, true);
    }
    if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") {
        return (3, true);
    }
    if let Some(m) = number_after("rx") {
        let tier = match m {
            5000..=5999 => 4,
            6000..=6599 | 7000..=7599 => 4,
            6600..=6999 | 7600..=9999 => 5,
            _ => 3,
        };
        return (tier, true);
    }
    if n.contains("radeon pro") {
        return (
            if number_after("pro").is_some_and(|m| m >= 5000) {
                4
            } else {
                3
            },
            true,
        );
    }
    if n.contains("radeon") || n.contains("amd") {
        // Integrated Radeon: 680M/780M/880M class on Ryzen 6000 and later.
        if let Some(m) = number_after("radeon") {
            if (600..=999).contains(&m) {
                return (if m >= 880 { 4 } else { 3 }, false);
            }
        }
        if n.contains("vega") {
            return (2, false);
        }
        let newer = cpu.summary.contains("Ryzen") && cpu.tier.is_some_and(|t| t >= 4);
        return (if newer { 3 } else { 2 }, false);
    }
    if n.contains("arc") {
        if let Some(m) = number_after("arc a") {
            return (if m >= 500 { 4 } else { 3 }, true);
        }
        return (3, false);
    }
    if n.contains("iris xe") {
        return (2, false);
    }
    if n.contains("intel") || n.contains("uhd") || n.contains("hd graphics") || n.contains("iris") {
        // An unnamed Intel adapter on a recent processor is at least Iris Xe class.
        let generic = !n.contains("hd") && !n.contains("iris");
        return (
            if generic && cpu.tier.is_some_and(|t| t >= 4) {
                2
            } else {
                1
            },
            false,
        );
    }
    if n.contains("adreno") || n.contains("qualcomm") {
        return (3, false);
    }
    (1, false)
}

// ---------------------------------------------------------- fit for purpose

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCase {
    Everyday,
    Work,
    Study,
    Programming,
    Creative,
    Video,
    Gaming,
}

pub const USE_CASES: [UseCase; 7] = [
    UseCase::Everyday,
    UseCase::Work,
    UseCase::Study,
    UseCase::Programming,
    UseCase::Creative,
    UseCase::Video,
    UseCase::Gaming,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    Great,
    Good,
    Limited,
    Poor,
    /// A lock or management signal: do not buy until it is removed.
    Blocked,
    /// Too little was read to say.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Good,
    Fair,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reason {
    pub tone: Tone,
    pub provenance: Provenance,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseFit {
    pub use_case: UseCase,
    pub fit: Fit,
    /// 0–100, `None` when [`Fit::Unknown`].
    pub score: Option<u8>,
    /// The most important reasons first: condition limits, then shortfalls.
    pub reasons: Vec<Reason>,
    /// What the scan did not read that this use depends on.
    pub unknowns: Vec<String>,
}

#[derive(Clone, Copy)]
enum Dim {
    Cpu,
    Ram,
    Storage,
    Graphics,
    Battery,
    Display,
}

/// What one kind of use needs: per dimension a weight and the minimum and
/// ideal levels (tiers, GB, display height). Weight 0 means it does not matter.
struct Profile {
    activity: &'static str,
    cpu: (u32, u8, u8),
    ram: (u32, u32, u32),
    /// Weight and minimum capacity in GB.
    storage: (u32, u32),
    gpu: (u32, u8, u8),
    battery: u32,
    display: (u32, u16, u16),
}

fn profile(u: UseCase) -> Profile {
    match u {
        UseCase::Everyday => Profile {
            activity: "browsing, email and streaming",
            cpu: (3, 1, 3),
            ram: (2, 4, 8),
            storage: (2, 128),
            gpu: (0, 1, 1),
            battery: 2,
            display: (1, 768, 1080),
        },
        UseCase::Work => Profile {
            activity: "office apps and video calls",
            cpu: (3, 2, 4),
            ram: (3, 8, 16),
            storage: (2, 256),
            gpu: (0, 1, 1),
            battery: 2,
            display: (1, 1080, 1080),
        },
        UseCase::Study => Profile {
            activity: "schoolwork and a day of classes",
            cpu: (2, 2, 3),
            ram: (2, 8, 8),
            storage: (1, 128),
            gpu: (0, 1, 1),
            battery: 4,
            display: (1, 768, 1080),
        },
        UseCase::Programming => Profile {
            activity: "coding, builds and dev tools",
            cpu: (4, 3, 5),
            ram: (4, 16, 32),
            storage: (3, 512),
            gpu: (0, 1, 1),
            battery: 1,
            display: (1, 1080, 1440),
        },
        UseCase::Creative => Profile {
            activity: "photo editing and design",
            cpu: (3, 3, 4),
            ram: (3, 16, 32),
            storage: (2, 512),
            gpu: (2, 2, 3),
            battery: 1,
            display: (3, 1080, 1440),
        },
        UseCase::Video => Profile {
            activity: "video editing and 3D",
            cpu: (4, 4, 5),
            ram: (3, 16, 32),
            storage: (2, 1000),
            gpu: (4, 3, 4),
            battery: 0,
            display: (1, 1080, 1440),
        },
        UseCase::Gaming => Profile {
            activity: "modern games",
            cpu: (3, 3, 4),
            ram: (2, 8, 16),
            storage: (2, 512),
            gpu: (6, 3, 5),
            battery: 0,
            display: (1, 1080, 1080),
        },
    }
}

/// 100 at or above ideal, 65–99 between minimum and ideal, 40 when just
/// short of the minimum (at or above `near`), 15 when far below it.
fn level_score(have: f64, min: f64, ideal: f64, near: f64) -> u8 {
    if have >= ideal {
        100
    } else if have >= min {
        if ideal > min {
            (65.0 + 35.0 * (have - min) / (ideal - min)).round() as u8
        } else {
            100
        }
    } else if have >= near {
        40
    } else {
        15
    }
}

fn fit_for(use_case: UseCase, scan: &Scan, grade: &Grade, specs: &Specs) -> UseFit {
    let p = profile(use_case);
    let activity = p.activity;
    let mut parts: Vec<(Dim, u32, u8)> = Vec::new();
    let mut reasons: Vec<(u8, Reason)> = Vec::new(); // (priority, reason)
    let mut unknowns = Vec::new();
    let inferred = |tone, text: String| Reason {
        tone,
        provenance: Provenance::Inferred,
        text,
    };
    let measured = |tone, text: String| Reason {
        tone,
        provenance: Provenance::Measured,
        text,
    };
    let tone_of = |s: u8| match s {
        85.. => Tone::Good,
        55..=84 => Tone::Fair,
        _ => Tone::Bad,
    };

    // Processor.
    let (w, min, ideal) = p.cpu;
    match specs.cpu.tier {
        Some(t) => {
            let s = level_score(t as f64, min as f64, ideal as f64, min as f64 - 1.0);
            parts.push((Dim::Cpu, w, s));
            let text = match tone_of(s) {
                Tone::Good => format!("{}: quick enough for {activity}", specs.cpu.summary),
                Tone::Fair => format!(
                    "{}: copes with {activity}, heavier jobs will feel slow",
                    specs.cpu.summary
                ),
                Tone::Bad => format!("{}: too slow for {activity}", specs.cpu.summary),
            };
            reasons.push((w as u8 * 2, inferred(tone_of(s), text)));
        }
        None => unknowns.push("Processor model".to_string()),
    }

    // Memory.
    let (w, min, ideal) = p.ram;
    match specs.ram_gb {
        Some(gb) => {
            let s = level_score(gb as f64, min as f64, ideal as f64, min as f64 / 2.0);
            parts.push((Dim::Ram, w, s));
            let text = match tone_of(s) {
                Tone::Good => format!("About {gb} GB of memory: plenty for {activity}"),
                Tone::Fair => format!("About {gb} GB of memory: enough, but many apps or tabs at once will slow it down"),
                Tone::Bad => format!("About {gb} GB of memory: too little for {activity} ({min} GB or more recommended)"),
            };
            reasons.push((w as u8 * 2, measured(tone_of(s), text)));
        }
        None => unknowns.push("Memory size".to_string()),
    }

    // Storage: speed times capacity.
    let (w, min_gb) = p.storage;
    match specs.storage.kind {
        Some(kind) if kind != StorageKind::Unknown || specs.storage.capacity_bytes.is_some() => {
            let speed = match kind {
                StorageKind::Nvme => 100,
                StorageKind::SataSsd => 85,
                StorageKind::Unknown => 75,
                StorageKind::Hdd => 25,
            };
            let gb = specs.storage.capacity_bytes.map(|b| b / 1_000_000_000);
            let room = match gb {
                Some(g) if g as u32 >= min_gb * 2 => 100,
                Some(g) if g as u32 + min_gb / 10 >= min_gb => 85,
                Some(g) if g as u32 >= min_gb / 2 => 55,
                Some(_) => 30,
                None => 80,
            };
            let s = (speed * room / 100) as u8;
            parts.push((Dim::Storage, w, s));
            let size = gb
                .map(|g| {
                    if g >= 1000 {
                        format!("{:.1} TB", g as f64 / 1000.0)
                    } else {
                        format!("{g} GB")
                    }
                })
                .unwrap_or_else(|| "size not reported".into());
            let text = match kind {
                StorageKind::Hdd => {
                    format!("Spinning hard drive ({size}): slow to start and open apps")
                }
                _ if room < 60 => {
                    format!("Only {size} of storage: fills up quickly with {activity}")
                }
                StorageKind::Nvme => format!("Fast NVMe SSD, {size}"),
                StorageKind::SataSsd => format!("SATA SSD, {size}: quick for most tasks"),
                StorageKind::Unknown => format!("Internal drive, {size}"),
            };
            reasons.push((w as u8 * 2, measured(tone_of(s), text)));
        }
        _ => unknowns.push("Main drive type and size".to_string()),
    }

    // Graphics.
    let (w, min, ideal) = p.gpu;
    if w > 0 {
        match specs.gpu.tier {
            Some(t) => {
                let s = level_score(t as f64, min as f64, ideal as f64, min as f64 - 1.0);
                parts.push((Dim::Graphics, w, s));
                let name = specs.gpu.name.clone().unwrap_or_else(|| "Graphics".into());
                let text = if use_case == UseCase::Gaming {
                    match t {
                        5 => format!("{name}: recent games at high settings"),
                        4 => format!("{name}: recent games at 1080p, medium to high settings"),
                        3 => format!("{name}: lighter and older games at 1080p, low settings"),
                        2 => format!("{name}: casual and indie games only"),
                        _ => format!("{name}: not made for gaming"),
                    }
                } else {
                    match tone_of(s) {
                        Tone::Good => format!("{name}: strong enough for {activity}"),
                        Tone::Fair => format!(
                            "{name}: workable for {activity}, effects and exports will be slower"
                        ),
                        Tone::Bad => format!("{name}: too weak for {activity}"),
                    }
                };
                reasons.push((w as u8 * 2, inferred(tone_of(s), text)));
            }
            None => unknowns.push("Graphics chip".to_string()),
        }
    }

    // Battery.
    let w = p.battery;
    if w > 0 {
        match specs.battery_percent {
            Some(pct) => {
                let s = match pct {
                    85.. => 100,
                    80..=84 => 90,
                    70..=79 => 65,
                    60..=69 => 40,
                    _ => 15,
                };
                parts.push((Dim::Battery, w, s));
                let text = match s {
                    90.. => format!("Battery holds {pct}% of its original capacity: close to new"),
                    65..=89 => format!("Battery holds {pct}% of its original capacity: expect somewhat shorter time unplugged"),
                    _ => format!("Battery holds {pct}% of its original capacity: plan on staying near a charger, or a replacement"),
                };
                reasons.push((w as u8 * 2, measured(tone_of(s), text)));
            }
            None => unknowns.push("Battery capacity".to_string()),
        }
    }

    // Display.
    let (w, min, ideal) = p.display;
    match specs.display {
        Some(r) => {
            let s = if r.height >= ideal {
                100
            } else if r.height >= min {
                75
            } else {
                35
            };
            parts.push((Dim::Display, w, s));
            let text = match s {
                100 => format!("{}×{} screen: sharp and roomy", r.width, r.height),
                75 => format!("{}×{} screen: fine, if not the sharpest", r.width, r.height),
                _ => format!(
                    "{}×{} screen: low resolution, text and photos look coarse",
                    r.width, r.height
                ),
            };
            reasons.push((
                w as u8,
                Reason {
                    tone: tone_of(s),
                    provenance: Provenance::Claimed,
                    text,
                },
            ));
        }
        None => unknowns.push("Screen resolution".to_string()),
    }

    if verdict_of_axis(grade, Axis::Encumbrance) == Verdict::NotGraded {
        unknowns.push("Whether the laptop is locked or managed by someone else".to_string());
    }

    let known: Vec<_> = parts
        .iter()
        .filter(|(d, _, _)| matches!(d, Dim::Cpu | Dim::Ram))
        .collect();
    if known.is_empty() {
        return UseFit {
            use_case,
            fit: Fit::Unknown,
            score: None,
            reasons: reasons.into_iter().map(|(_, r)| r).collect(),
            unknowns,
        };
    }

    let total_w: u32 = parts.iter().map(|(_, w, _)| w).sum();
    let mut score = (parts.iter().map(|(_, w, s)| w * *s as u32).sum::<u32>() as f64
        / total_w.max(1) as f64)
        .round() as u8;

    // A key part below the minimum limits the whole: a fast processor does
    // not make up for 4 GB of memory.
    // Far below the minimum on a key part is a poor match outright.
    if parts.iter().any(|(_, w, s)| *w >= 3 && *s < 30) {
        score = score.min(40);
    } else if parts.iter().any(|(_, w, s)| *w >= 3 && *s < 50) {
        score = score.min(50);
    } else if parts.iter().any(|(_, w, s)| *w > 0 && *s < 50) {
        score = score.min(64);
    }
    // Without its most important part, a use cannot be rated better than limited.
    if (p.gpu.0 >= 4 && specs.gpu.tier.is_none()) || (p.cpu.0 >= 4 && specs.cpu.tier.is_none()) {
        score = score.min(64);
    }

    // Platform limits buyers often discover too late.
    if use_case == UseCase::Gaming && (specs.cpu.apple || scan.os == "macos") {
        score = score.min(55);
        reasons.push((
            50,
            inferred(
                Tone::Bad,
                "Many popular PC games are not released for Mac".into(),
            ),
        ));
    } else if specs.cpu.arm
        && !specs.cpu.apple
        && matches!(use_case, UseCase::Gaming | UseCase::Programming)
    {
        score = score.min(60);
        reasons.push((
            50,
            inferred(
                Tone::Fair,
                "Arm processor: some games, anti-cheat and developer tools do not run natively"
                    .into(),
            ),
        ));
    }

    // Condition limits every use.
    let verdict_of = |a: Axis| verdict_of_axis(grade, a);
    let mut fit_override = None;
    if verdict_of(Axis::Encumbrance) == Verdict::Red {
        score = score.min(10);
        fit_override = Some(Fit::Blocked);
        reasons.push((100, measured(Tone::Bad, "Someone else may still control this laptop. Do not buy until the seller removes it".into())));
    }
    match verdict_of(Axis::Function) {
        Verdict::Red => {
            score = score.min(25);
            reasons.push((
                90,
                measured(
                    Tone::Bad,
                    "A drive shows failure signs: replace it before trusting it with your files"
                        .into(),
                ),
            ));
        }
        Verdict::Amber => {
            score = score.min(79);
            reasons.push((
                80,
                measured(
                    Tone::Fair,
                    "A drive needs attention: fine for now, budget for a replacement".into(),
                ),
            ));
        }
        _ => {}
    }
    if verdict_of(Axis::Battery) == Verdict::Red && p.battery >= 2 {
        score = score.min(55);
    }

    let fit = fit_override.unwrap_or(fit_from_score(score));
    // Strongest signals first; within a priority, problems before strengths.
    reasons.sort_by_key(|(prio, r)| {
        (
            std::cmp::Reverse(*prio),
            match r.tone {
                Tone::Bad => 0,
                Tone::Fair => 1,
                Tone::Good => 2,
            },
        )
    });
    UseFit {
        use_case,
        fit,
        score: Some(score),
        reasons: reasons.into_iter().map(|(_, r)| r).collect(),
        unknowns,
    }
}

fn verdict_of_axis(grade: &Grade, axis: Axis) -> Verdict {
    grade
        .axes
        .iter()
        .find(|x| x.axis == axis)
        .map(|x| x.verdict)
        .unwrap_or(Verdict::NotGraded)
}

fn tips(scan: &Scan, grade: &Grade, specs: &Specs) -> Vec<String> {
    let mut out = Vec::new();
    let verdict_of = |a: Axis| verdict_of_axis(grade, a);
    if verdict_of(Axis::Encumbrance) == Verdict::Red {
        out.push("Ask the seller to remove the management or account lock, then scan again before paying.".into());
    } else if verdict_of(Axis::Encumbrance) == Verdict::Amber {
        out.push("This looks like an ex-company laptop. Ask the seller for proof it was released by its previous owner.".into());
    }
    if verdict_of(Axis::Function) == Verdict::Red {
        out.push("Back up nothing important to this drive. Factor the cost of a new drive into your offer.".into());
    }
    if specs.storage.kind == Some(StorageKind::Hdd) {
        out.push("Swapping the hard drive for an SSD is the single biggest speed upgrade for this laptop.".into());
    }
    if specs.battery_percent.is_some_and(|p| p < 70) {
        out.push("The battery is well worn. Check whether a replacement is available for this model, and use it in your price talk.".into());
    }
    if specs.ram_gb.is_some_and(|g| g < 8) {
        out.push("Memory is low. Ask whether it can be upgraded: on many thin laptops it is soldered in.".into());
    }
    if !scan.elevated {
        out.push("Scan again with administrator rights to read drive health and serials.".into());
    }
    if scan.os == "linux" {
        out.push(
            "Run the scan from the laptop's own Windows or macOS to check for management locks."
                .into(),
        );
    }
    out.push(
        "Scan again a few days later: a second scan makes the result much more reliable.".into(),
    );
    out
}

pub fn assess(scan: &Scan, grade: &Grade) -> Assessment {
    assess_with(scan, grade, None)
}

/// The assessment with hands-on test results taken into account. A test
/// that found a problem holds the condition score to 79 and limits every use
/// it matters to; the problems lead the reasons and the next steps.
pub fn assess_with(scan: &Scan, grade: &Grade, hands_on: Option<&HandsOn>) -> Assessment {
    let mut a = assess_scan(scan, grade);
    let Some(h) = hands_on else {
        a.tips.insert(
            a.tips.len().saturating_sub(1),
            "Run the hands-on tests: keyboard, touchpad, screen and speakers take about five minutes.".into(),
        );
        return a;
    };
    let finished: Vec<_> = h
        .tests
        .iter()
        .filter(|t| t.outcome != Outcome::Incomplete)
        .collect();
    let failed: Vec<_> = h.failed().collect();
    let problems: Vec<String> = failed
        .iter()
        .flat_map(|t| {
            let label = handson::kind_label(t.kind);
            if t.problems.is_empty() {
                vec![format!("{label}: {}", t.summary)]
            } else {
                t.problems.iter().map(|p| format!("{label}: {p}")).collect()
            }
        })
        .collect();

    if !failed.is_empty() {
        let c = &mut a.condition;
        if let Some(score) = c.score {
            if score > 79 {
                c.score = Some(79);
                c.cap = Some(c.cap.map_or(79, |cap| cap.min(79)));
                c.band = Some(Band::Good);
            }
        }
        for u in a.uses.iter_mut() {
            let relevant: Vec<_> = failed
                .iter()
                .filter(|t| t.kind != TestKind::Speakers || speakers_matter(u.use_case))
                .collect();
            if relevant.is_empty() {
                continue;
            }
            for t in relevant.iter().rev() {
                u.reasons.insert(
                    0,
                    Reason {
                        tone: Tone::Bad,
                        provenance: Provenance::Measured,
                        text: format!("{} test: {}", handson::kind_label(t.kind), t.summary),
                    },
                );
            }
            if let Some(score) = u.score {
                u.score = Some(score.min(64));
                if !matches!(u.fit, Fit::Blocked | Fit::Unknown) {
                    u.fit = fit_from_score(score.min(64));
                }
            }
        }
        let mut best: Vec<&UseFit> = a
            .uses
            .iter()
            .filter(|u| matches!(u.fit, Fit::Great | Fit::Good))
            .collect();
        best.sort_by_key(|u| std::cmp::Reverse(u.score));
        a.best_for = best.iter().map(|u| u.use_case).collect();
        let at = a
            .tips
            .iter()
            .take_while(|t| t.contains("lock") || t.contains("ex-company"))
            .count();
        a.tips.insert(
            at,
            format!(
                "The hands-on tests found problems ({}). Ask the seller about them and adjust your offer.",
                problems.join("; ")
            ),
        );
    }
    let not_run: Vec<TestKind> = ALL_TESTS
        .into_iter()
        .filter(|k| !finished.iter().any(|t| t.kind == *k))
        .collect();
    if !not_run.is_empty() {
        let names: Vec<&str> = not_run.iter().map(|k| handson::kind_label(*k)).collect();
        a.tips.insert(
            a.tips.len().saturating_sub(1),
            format!(
                "Finish the hands-on tests: {} not run yet.",
                names.join(", ")
            ),
        );
    }
    a.hands_on = Some(HandsOnSummary {
        finished: finished.len(),
        passed: finished
            .iter()
            .filter(|t| t.outcome == Outcome::Passed)
            .count(),
        problems,
        not_run,
    });
    a
}

fn speakers_matter(u: UseCase) -> bool {
    !matches!(u, UseCase::Programming | UseCase::Creative)
}

fn fit_from_score(score: u8) -> Fit {
    match score {
        80.. => Fit::Great,
        65..=79 => Fit::Good,
        45..=64 => Fit::Limited,
        _ => Fit::Poor,
    }
}

fn assess_scan(scan: &Scan, grade: &Grade) -> Assessment {
    let specs = specs(scan);
    let uses: Vec<UseFit> = USE_CASES
        .iter()
        .map(|&u| fit_for(u, scan, grade, &specs))
        .collect();
    let mut best: Vec<&UseFit> = uses
        .iter()
        .filter(|u| matches!(u.fit, Fit::Great | Fit::Good))
        .collect();
    best.sort_by_key(|u| std::cmp::Reverse(u.score));
    Assessment {
        version: ASSESS_VERSION.to_string(),
        condition: condition(scan, grade),
        best_for: best.iter().map(|u| u.use_case).collect(),
        tips: tips(scan, grade, &specs),
        specs,
        uses,
        hands_on: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::signals;
    use crate::grade::grade;

    fn tier(brand: &str, cores: u32) -> Option<u8> {
        cpu_estimate(Some(brand), Some(cores)).tier
    }

    #[test]
    fn processor_tiers() {
        let cases = [
            ("Intel(R) Core(TM) i7-7600U CPU @ 2.80GHz", 2, 2),
            ("Intel(R) Core(TM) i5-4300U CPU @ 1.90GHz", 2, 2),
            ("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz", 4, 3),
            ("Intel(R) Core(TM) i7-1165G7 @ 2.80GHz", 4, 4),
            ("Intel(R) Core(TM) i7-10750H CPU @ 2.60GHz", 6, 4),
            ("12th Gen Intel(R) Core(TM) i7-12700H", 14, 5),
            ("13th Gen Intel(R) Core(TM) i5-1335U", 10, 4),
            ("Intel(R) Core(TM) i3-1005G1 CPU @ 1.20GHz", 2, 2),
            ("Intel(R) Core(TM) Ultra 7 155H", 16, 5),
            ("Intel(R) Celeron(R) N4020 CPU @ 1.10GHz", 2, 1),
            ("Intel(R) N100", 4, 2),
            ("AMD Ryzen 5 3500U with Radeon Vega Mobile Gfx", 4, 3),
            ("AMD Ryzen 3 3200U with Radeon Vega Mobile Gfx", 2, 2),
            ("AMD Ryzen 7 5800H with Radeon Graphics", 8, 5),
            ("AMD Ryzen 5 5500U with Radeon Graphics", 6, 4),
            ("AMD Ryzen 7 PRO 4750U with Radeon Graphics", 8, 4),
            ("AMD Ryzen AI 9 HX 370 w/ Radeon 890M", 12, 5),
            ("AMD Athlon Silver 3050U with Radeon Graphics", 2, 1),
            ("Apple M1", 8, 4),
            ("Apple M2 Pro", 12, 5),
            ("Apple M3", 8, 5),
        ];
        for (brand, cores, want) in cases {
            assert_eq!(tier(brand, cores), Some(want), "{brand}");
        }
        let m1 = cpu_estimate(Some("Apple M1 Max"), Some(10));
        assert!(m1.apple && m1.arm);
        assert_eq!(m1.summary, "Apple M1 Max");
        assert_eq!(
            cpu_estimate(Some("Intel(R) Core(TM) i7-7600U CPU @ 2.80GHz"), Some(2)).summary,
            "Intel Core i7, 7th gen, low-power"
        );
        assert_eq!(cpu_estimate(None, None).tier, None);
        assert_eq!(
            cpu_estimate(Some("Mystery Chip 9000"), Some(8)).tier,
            Some(4)
        );
        for junk in [
            "",
            "-",
            "i7-",
            "Core i",
            "ryzen",
            "ryzen ai",
            "apple m",
            "Core Ultra",
            "Apple Mx Pro",
        ] {
            let _ = cpu_estimate(Some(junk), None);
        }
    }

    #[test]
    fn graphics_tiers() {
        let intel = cpu_estimate(Some("Intel(R) Core(TM) i7-1165G7"), Some(4));
        let cases = [
            ("NVIDIA GeForce RTX 3050 Laptop GPU", 4, true),
            ("NVIDIA GeForce RTX 4070 Laptop GPU", 5, true),
            ("NVIDIA GeForce GTX 1650 Mobile / Max-Q", 3, true),
            ("NVIDIA GeForce MX450", 2, true),
            ("NVIDIA RTX A2000 Laptop GPU", 3, true),
            ("AMD Radeon RX 6600M", 5, true),
            ("AMD Radeon Pro 5500M", 4, true),
            ("AMD Radeon 780M", 3, false),
            (
                "AMD Radeon Vega Series / Radeon Vega Mobile Series",
                2,
                false,
            ),
            ("Intel Iris Xe Graphics", 2, false),
            ("Intel UHD Graphics 620", 1, false),
            ("Intel graphics", 2, false),
            ("Intel Arc A370M", 3, true),
            ("Intel Arc Graphics", 3, false),
            ("Apple M1", 3, false),
            ("Apple M2 Max", 5, false),
            ("Virtio graphics", 1, false),
        ];
        for (name, want, discrete) in cases {
            assert_eq!(gpu_tier(name, &intel), (want, discrete), "{name}");
        }
        let gpus = [
            Gpu {
                name: Field::claimed(Some("Intel UHD Graphics 630".into()), "t"),
                ..Gpu::default()
            },
            Gpu {
                name: Field::claimed(Some("NVIDIA GeForce GTX 1650".into()), "t"),
                ..Gpu::default()
            },
        ];
        let g = gpu_estimate(&gpus, &intel);
        assert_eq!(
            (g.tier, g.discrete),
            (Some(3), Some(true)),
            "strongest adapter wins"
        );
        assert_eq!(
            gpu_estimate(&[], &intel).tier,
            None,
            "none recorded is unknown"
        );
    }

    #[test]
    fn installed_memory_from_usable() {
        let gib = |g: f64| (g * (1u64 << 30) as f64) as u64;
        assert_eq!(installed_gb(gib(15.6)), 16);
        assert_eq!(installed_gb(gib(7.7)), 8);
        assert_eq!(installed_gb(gib(3.76)), 4);
        assert_eq!(installed_gb(gib(31.2)), 32);
    }

    fn nvme(used: u8, media_errors: u128) -> Drive {
        Drive {
            name: "nvme0n1".into(),
            transport: Transport::Nvme,
            rotational: Field::measured(Some(false), "t"),
            removable: false,
            model: Field::default(),
            serial: Field::default(),
            firmware: Field::default(),
            capacity_bytes: Field::claimed(Some(512_000_000_000), "t"),
            health: DriveHealth::Nvme(NvmeHealth {
                available_spare: 100,
                available_spare_threshold: 10,
                percentage_used: used,
                power_on_hours: 2_000,
                data_units_written: 20_000_000,
                media_errors,
                ..NvmeHealth::default()
            }),
        }
    }

    fn battery(design: u64, full: u64) -> Battery {
        let cap = |v| {
            Field::measured(
                Some(Capacity {
                    value: v,
                    unit: CapacityUnit::MilliwattHours,
                }),
                "t",
            )
        };
        Battery {
            name: "BAT0".into(),
            design_capacity: cap(design),
            full_charge_capacity: cap(full),
            cycle_count: Field::measured(Some(200), "t"),
            ..Battery::default()
        }
    }

    fn clean_signals() -> Vec<EncumbranceSignal> {
        vec![EncumbranceSignal {
            id: signals::MDM_ENROLMENT.into(),
            present: false,
            provenance: Provenance::Claimed,
            source: "t".into(),
            detail: "no MDM enrolment recorded".into(),
        }]
    }

    /// A 2022 work laptop in good order, scanned from Windows.
    fn machine() -> Scan {
        Scan {
            os: "windows".into(),
            elevated: true,
            cpu: Cpu {
                brand: Field::measured(Some("12th Gen Intel(R) Core(TM) i7-1260P".into()), "t"),
                logical_cores: Field::measured(Some(16), "t"),
                physical_cores: Field::measured(Some(12), "t"),
            },
            memory: Memory {
                total_bytes: Field::measured(Some(15_800_000_000), "t"),
            },
            storage: vec![nvme(3, 0)],
            batteries: vec![battery(57_000, 52_000)],
            gpus: vec![Gpu {
                name: Field::claimed(Some("Intel Iris Xe Graphics".into()), "t"),
                ..Gpu::default()
            }],
            displays: vec![Display {
                connector: "eDP-1".into(),
                native_width: Some(1920),
                native_height: Some(1200),
                ..Display::default()
            }],
            encumbrance: clean_signals(),
            ..Scan::default()
        }
    }

    fn assessed(scan: &Scan) -> Assessment {
        assess(scan, &grade(scan))
    }

    fn fit(a: &Assessment, u: UseCase) -> &UseFit {
        a.uses.iter().find(|f| f.use_case == u).unwrap()
    }

    #[test]
    fn clean_machine_scores_high_from_all_three_areas() {
        let a = assessed(&machine());
        let c = &a.condition;
        assert_eq!(c.scored_areas, 3);
        assert_eq!(c.cap, None);
        // Drives 97, battery 91, locks 100, weighted 45/25/30.
        assert_eq!(c.score, Some(96));
        assert_eq!(c.band, Some(Band::Excellent));
        assert_eq!(a.uses.len(), USE_CASES.len());
        assert!(matches!(fit(&a, UseCase::Work).fit, Fit::Great));
        assert!(a.best_for.contains(&UseCase::Everyday));
        assert!(
            !matches!(fit(&a, UseCase::Gaming).fit, Fit::Great | Fit::Good),
            "integrated graphics is not a gaming machine"
        );
    }

    #[test]
    fn a_red_finding_caps_the_score_whatever_else_is_good() {
        let mut s = machine();
        s.storage = vec![nvme(3, 1)];
        let a = assessed(&s);
        assert_eq!(a.condition.cap, Some(39));
        assert!(a.condition.score.unwrap() <= 39);
        assert_eq!(a.condition.band, Some(Band::Poor));
        let work = fit(&a, UseCase::Work);
        assert!(work.score.unwrap() <= 25);
        assert!(work.reasons[0].text.contains("failure signs"));
    }

    #[test]
    fn a_lock_blocks_every_use() {
        let mut s = machine();
        s.encumbrance[0].present = true;
        s.encumbrance[0].detail = "enrolled in MDM (Contoso)".into();
        let a = assessed(&s);
        assert!(a.uses.iter().all(|u| u.fit == Fit::Blocked));
        assert!(a.best_for.is_empty());
        assert!(a.tips[0].contains("remove"));
        let locks = a
            .condition
            .areas
            .iter()
            .find(|x| x.axis == Axis::Encumbrance)
            .unwrap();
        assert_eq!(locks.score, Some(0));
    }

    #[test]
    fn missing_evidence_is_left_out_not_counted_as_full_marks() {
        let mut s = machine();
        s.os = "linux".into();
        s.encumbrance = vec![];
        s.batteries = vec![battery(50_000, 50_000)]; // exact match: not reported
        let a = assessed(&s);
        assert_eq!(a.condition.scored_areas, 1);
        let battery = a
            .condition
            .areas
            .iter()
            .find(|x| x.axis == Axis::Battery)
            .unwrap();
        assert_eq!(battery.score, None);
        assert!(battery.summary.starts_with("Not scored"));
        let locks = a
            .condition
            .areas
            .iter()
            .find(|x| x.axis == Axis::Encumbrance)
            .unwrap();
        assert!(locks.summary.contains("Linux"));
        assert!(fit(&a, UseCase::Study)
            .unknowns
            .iter()
            .any(|u| u.contains("Battery")));
        assert!(fit(&a, UseCase::Work)
            .unknowns
            .iter()
            .any(|u| u.contains("locked")));

        let nothing = assessed(&Scan::default());
        assert_eq!(nothing.condition.score, None);
        assert!(nothing
            .uses
            .iter()
            .all(|u| u.fit == Fit::Unknown && u.score.is_none()));
    }

    #[test]
    fn weak_parts_limit_the_uses_that_need_them() {
        let mut s = machine();
        s.cpu.brand = Field::measured(Some("Intel(R) Celeron(R) N4020 CPU @ 1.10GHz".into()), "t");
        s.cpu.physical_cores = Field::measured(Some(2), "t");
        s.memory.total_bytes = Field::measured(Some(3_900_000_000), "t");
        let mut hdd = nvme(0, 0);
        hdd.transport = Transport::Sata;
        hdd.rotational = Field::measured(Some(true), "t");
        hdd.health = DriveHealth::Ata(AtaHealth {
            attributes: vec![AtaAttribute {
                id: 5,
                current: 100,
                worst: 100,
                raw: 0,
            }],
        });
        s.storage = vec![hdd];
        let a = assessed(&s);
        assert_eq!(fit(&a, UseCase::Programming).fit, Fit::Poor);
        assert!(fit(&a, UseCase::Everyday).score > fit(&a, UseCase::Video).score);
        assert!(a.tips.iter().any(|t| t.contains("SSD")));
        assert!(a.tips.iter().any(|t| t.contains("Memory is low")));
    }

    #[test]
    fn a_mac_is_not_recommended_for_pc_gaming() {
        let mut s = machine();
        s.os = "macos".into();
        s.cpu.brand = Field::measured(Some("Apple M2 Max".into()), "t");
        s.gpus = vec![Gpu {
            name: Field::claimed(Some("Apple M2 Max".into()), "t"),
            ..Gpu::default()
        }];
        let a = assessed(&s);
        let g = fit(&a, UseCase::Gaming);
        assert!(g.score.unwrap() <= 55);
        assert!(g.reasons.iter().any(|r| r.text.contains("Mac")));
        assert_eq!(fit(&a, UseCase::Video).fit, Fit::Great);
    }

    fn hands_on(kind: TestKind, outcome: Outcome, problems: &[&str]) -> HandsOn {
        let mut h = HandsOn::default();
        h.record(crate::handson::TestResult {
            kind,
            outcome,
            tested_at: 1,
            summary: if problems.is_empty() {
                "All keys registered".into()
            } else {
                "2 keys did not register".into()
            },
            problems: problems.iter().map(|p| p.to_string()).collect(),
            detail: serde_json::Value::Null,
        });
        h
    }

    #[test]
    fn a_failed_hands_on_test_caps_the_score_and_leads_the_reasons() {
        let s = machine();
        let g = grade(&s);
        let h = hands_on(
            TestKind::Keyboard,
            Outcome::Problems,
            &["Key K did not register", "Key L did not register"],
        );
        let a = assess_with(&s, &g, Some(&h));
        assert_eq!(a.condition.score, Some(79));
        assert_eq!(a.condition.cap, Some(79));
        let work = fit(&a, UseCase::Work);
        assert_eq!(work.fit, Fit::Limited);
        assert!(work.reasons[0].text.starts_with("Keyboard test"));
        assert!(a
            .tips
            .iter()
            .any(|t| t.contains("Keyboard: Key K did not register")));
        let summary = a.hands_on.unwrap();
        assert_eq!((summary.finished, summary.passed), (1, 0));
        assert_eq!(summary.not_run.len(), 4);
    }

    #[test]
    fn speakers_do_not_limit_coding_and_passes_change_nothing() {
        let s = machine();
        let g = grade(&s);
        let plain = assess(&s, &g);
        let speakers = hands_on(
            TestKind::Speakers,
            Outcome::Problems,
            &["Right speaker silent"],
        );
        let a = assess_with(&s, &g, Some(&speakers));
        assert_eq!(
            fit(&a, UseCase::Programming).score,
            fit(&plain, UseCase::Programming).score
        );
        assert!(fit(&a, UseCase::Everyday).score.unwrap() <= 64);

        let passed = hands_on(TestKind::Keyboard, Outcome::Passed, &[]);
        let a = assess_with(&s, &g, Some(&passed));
        assert_eq!(a.condition.score, plain.condition.score);
        assert!(plain
            .tips
            .iter()
            .any(|t| t.starts_with("Run the hands-on tests")));
        assert!(a
            .tips
            .iter()
            .any(|t| t.starts_with("Finish the hands-on tests")));
    }

    #[test]
    fn assessment_wording_never_clears_a_machine() {
        for s in [machine(), Scan::default()] {
            let text = serde_json::to_string(&assessed(&s)).unwrap().to_lowercase();
            assert!(!text.contains("healthy"));
            assert!(!text.contains("not stolen"));
        }
    }
}
