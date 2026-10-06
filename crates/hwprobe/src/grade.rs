//! The local grading engine.
//!
//! Three separate axes (function, battery, encumbrance), each with its own
//! verdict. The headline is the worst axis: a perfect CPU never averages away
//! a drive with pending sectors. There is no weighted score.
//!
//! Every grade carries [`GRADER_VERSION`]. Grades from different versions are
//! not comparable; bump the version whenever a rule or threshold changes.

use serde::{Deserialize, Serialize};

use crate::model::*;

pub const GRADER_VERSION: &str = "0.2.0";

/// Ordered from best to worst so `max()` picks the limiting factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// No evidence either way. Never shown as a pass.
    NotGraded,
    Green,
    Amber,
    Red,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Function,
    Battery,
    Encumbrance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub component: String,
    pub verdict: Verdict,
    pub provenance: Provenance,
    pub message: String,
    /// The rule and its evidence basis, so a grade can be contested.
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisGrade {
    pub axis: Axis,
    pub verdict: Verdict,
    /// Number of checks that produced a verdict, for "no faults in N checks".
    pub checks: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grade {
    pub grader_version: String,
    /// Worst verdict across the graded axes.
    pub headline: Verdict,
    pub limiting_axis: Option<Axis>,
    /// False when any axis could not be graded.
    pub complete: bool,
    pub axes: Vec<AxisGrade>,
    pub caveats: Vec<String>,
    /// Named checks software cannot perform. Listed explicitly because an
    /// omitted check reads as a passed one.
    pub not_assessable: Vec<String>,
}

pub const STORAGE_CAVEAT: &str = "In a large field study (Google, FAST 2007) about 36% of drives that failed showed zero SMART error counts beforehand. A clean storage result lowers risk; it does not clear the drive.";
pub const SNAPSHOT_CAVEAT: &str = "Single snapshot: confidence is limited. A second scan days later materially raises confidence.";
pub const BATTERY_CAVEAT: &str = "Battery health comes from the pack's own fuel gauge, which can be miscalibrated or reset. Swelling cannot be detected in software: check the case for bulging.";
pub const PROVENANCE_CAVEAT: &str = "This report does not and cannot determine whether the laptop is stolen. No public register of laptop serials exists.";
pub const CLAIMED_CAVEAT: &str = "Model, serial and capacity labels are read from rewritable firmware and are claims, not measurements.";

pub const NOT_ASSESSABLE: &[&str] = &[
    "Battery swelling",
    "Hinge, chassis and screw wear",
    "Liquid-damage history",
    "Keyboard actuation feel",
    "Undervolt or overclock history",
    "GPU mining or reball history",
    "Firmware (BIOS/UEFI) supervisor password",
    "Cloud registration held by the vendor (Windows Autopilot, Apple Business Manager)",
    "Theft status",
];

pub fn grade(scan: &Scan) -> Grade {
    let axes = vec![
        function_axis(scan),
        battery_axis(scan),
        encumbrance_axis(scan),
    ];
    let limiting = axes
        .iter()
        .filter(|a| a.verdict != Verdict::NotGraded)
        .max_by_key(|a| a.verdict);
    let headline = limiting.map(|a| a.verdict).unwrap_or(Verdict::NotGraded);

    let mut caveats = vec![SNAPSHOT_CAVEAT.to_string(), CLAIMED_CAVEAT.to_string()];
    if scan
        .storage
        .iter()
        .any(|d| !matches!(d.health, DriveHealth::Unavailable { .. }))
    {
        caveats.push(STORAGE_CAVEAT.to_string());
    }
    if !scan.batteries.is_empty() {
        caveats.push(BATTERY_CAVEAT.to_string());
    }
    caveats.push(PROVENANCE_CAVEAT.to_string());

    Grade {
        grader_version: GRADER_VERSION.to_string(),
        headline,
        limiting_axis: limiting
            .filter(|a| a.verdict > Verdict::Green)
            .map(|a| a.axis),
        complete: axes.iter().all(|a| a.verdict != Verdict::NotGraded),
        axes,
        caveats,
        not_assessable: NOT_ASSESSABLE.iter().map(|s| s.to_string()).collect(),
    }
}

fn axis(axis: Axis, findings: Vec<Finding>) -> AxisGrade {
    AxisGrade {
        axis,
        verdict: findings
            .iter()
            .map(|f| f.verdict)
            .max()
            .unwrap_or(Verdict::NotGraded),
        checks: findings
            .iter()
            .filter(|f| f.verdict != Verdict::NotGraded)
            .count(),
        findings,
    }
}

fn finding(
    component: &str,
    verdict: Verdict,
    provenance: Provenance,
    message: String,
    basis: &str,
) -> Finding {
    Finding {
        component: component.into(),
        verdict,
        provenance,
        message,
        basis: basis.into(),
    }
}

// ---------------------------------------------------------------- function

fn function_axis(scan: &Scan) -> AxisGrade {
    let mut findings = Vec::new();
    for drive in &scan.storage {
        storage_findings(drive, &mut findings);
    }
    axis(Axis::Function, findings)
}

fn storage_findings(drive: &Drive, out: &mut Vec<Finding>) {
    let c = format!("storage/{}", drive.name);
    let health = match (&drive.transport, &drive.health) {
        (Transport::Usb, _) => {
            out.push(finding(
                &c,
                Verdict::NotGraded,
                Provenance::NotAssessable,
                "External drive behind a USB bridge: health data unverified, not graded as internal storage".into(),
                "SMART over USB bridges is bridge-specific and frequently broken",
            ));
            return;
        }
        (_, DriveHealth::Unavailable { reason }) => {
            out.push(finding(
                &c,
                Verdict::NotGraded,
                Provenance::Measured,
                format!("Health not read: {reason}"),
                "missing evidence is never a pass",
            ));
            return;
        }
        (_, h) => h,
    };

    match health {
        DriveHealth::Nvme(h) => nvme_findings(&c, h, out),
        DriveHealth::Ata(h) => ata_findings(&c, h, drive, out),
        DriveHealth::Unavailable { .. } => unreachable!(),
    }
}

fn nvme_findings(c: &str, h: &NvmeHealth, out: &mut Vec<Finding>) {
    let m = Provenance::Measured;
    out.push(if h.critical_warning != 0 {
        finding(
            c,
            Verdict::Red,
            m,
            format!("NVMe critical warning set (0x{:02X})", h.critical_warning),
            "any nonzero critical_warning indicates a problem",
        )
    } else {
        finding(
            c,
            Verdict::Green,
            m,
            "No NVMe critical warning".into(),
            "critical_warning should be 0",
        )
    });

    out.push(if h.media_errors > 0 {
        finding(
            c,
            Verdict::Red,
            m,
            format!("{} uncorrectable media errors", h.media_errors),
            "one observed uncorrectable error is a persistent-defect signal (Meza et al.)",
        )
    } else {
        finding(
            c,
            Verdict::Green,
            m,
            "0 media errors".into(),
            "media_errors should be 0",
        )
    });

    if h.available_spare_threshold > 0 {
        out.push(if h.available_spare <= h.available_spare_threshold {
            finding(
                c,
                Verdict::Red,
                m,
                format!(
                    "Spare blocks at {}%, at or below the {}% alert floor",
                    h.available_spare, h.available_spare_threshold
                ),
                "threshold is the vendor's own alert floor",
            )
        } else {
            finding(
                c,
                Verdict::Green,
                m,
                format!("Spare blocks at {}%", h.available_spare),
                "available_spare above the vendor threshold",
            )
        });
    }

    let used = h.percentage_used as u32;
    let remaining_hi = 100u32.saturating_sub(used);
    out.push(match used {
        100.. => finding(
            c,
            Verdict::Amber,
            m,
            format!("Rated life consumed ({used}% used, vendor estimate)"),
            "percentage_used is a vendor wear estimate and may exceed 100; it is not a failure",
        ),
        80..=99 => finding(
            c,
            Verdict::Amber,
            m,
            format!("Drive life: about {remaining_hi}% remaining (vendor estimate)"),
            "wear odometer, not a health verdict",
        ),
        _ => finding(
            c,
            Verdict::Green,
            m,
            format!("Drive life: about {remaining_hi}% remaining (vendor estimate)"),
            "wear odometer, not a health verdict",
        ),
    });

    write_plausibility(c, h.power_on_hours, h.bytes_written(), out);
}

fn ata_findings(c: &str, h: &AtaHealth, drive: &Drive, out: &mut Vec<Finding>) {
    let m = Provenance::Measured;
    let count = |id| h.attribute(id).map(|a| a.count());

    let red: Vec<String> = [
        (197, "pending sectors"),
        (198, "offline-uncorrectable sectors"),
        (187, "reported uncorrectable errors"),
    ]
    .iter()
    .filter_map(|&(id, label)| {
        count(id)
            .filter(|&n| n > 0)
            .map(|n| format!("{n} {label} (attr {id})"))
    })
    .collect();
    let amber: Vec<String> = [(5, "reallocated sectors"), (188, "command timeouts")]
        .iter()
        .filter_map(|&(id, label)| {
            count(id)
                .filter(|&n| n > 0)
                .map(|n| format!("{n} {label} (attr {id})"))
        })
        .collect();
    let checked = [5u8, 187, 188, 197, 198]
        .iter()
        .filter(|&&id| h.attribute(id).is_some())
        .count();

    if !red.is_empty() {
        out.push(finding(c, Verdict::Red, m, format!("Do not trust this drive: {}", red.join(", ")), "Google FAST 2007: 16-21x failure risk, 39x within 60 days; Backblaze replaces on 187>0"));
    }
    if !amber.is_empty() {
        out.push(finding(
            c,
            Verdict::Amber,
            m,
            format!("Degrading, not dying: {}", amber.join(", ")),
            "Google FAST 2007: 3-6x baseline risk, ~85% survive 8 months",
        ));
    }
    if red.is_empty() && amber.is_empty() {
        out.push(if checked == 0 {
            finding(
                c,
                Verdict::NotGraded,
                m,
                "Drive reports none of the five failure-predicting SMART attributes".into(),
                "missing evidence is never a pass",
            )
        } else {
            finding(
                c,
                Verdict::Green,
                m,
                format!("No faults in {checked} of 5 failure-predicting SMART attributes"),
                "Backblaze attributes 5, 187, 188, 197, 198",
            )
        });
    }

    let poh = count(9).map(u128::from);
    let rotational = drive.rotational.get().copied().unwrap_or(false);
    if let (true, Some(hours)) = (rotational, poh) {
        if hours >= 87_600 {
            out.push(finding(
                c,
                Verdict::Amber,
                m,
                format!("Hard drive has {hours} power-on hours (over 10 years)"),
                "Backblaze 2025: AFR peaks around 10 years; age alone is not penalised before that",
            ));
        }
    }
}

/// Cross-check host writes against power-on hours. A drive whose counters
/// are physically implausible has likely been reset or reflashed.
/// These thresholds are this app's own convention, not a published standard.
fn write_plausibility(c: &str, hours: u128, bytes_written: u128, out: &mut Vec<Finding>) {
    const MB: u128 = 1_000_000;
    const GB: u128 = 1_000 * MB;
    if hours >= 10_000 && bytes_written < 100 * MB * hours {
        out.push(finding(
            c,
            Verdict::Amber,
            Provenance::Inferred,
            format!(
                "{hours} power-on hours but only {} GB written: counters may have been reset",
                bytes_written / GB
            ),
            "app convention: under 100 MB/h over 10,000+ hours is implausible",
        ));
    } else if hours >= 50 && bytes_written > 200 * GB * hours {
        out.push(finding(
            c,
            Verdict::Amber,
            Provenance::Inferred,
            format!(
                "{} TB written in {hours} power-on hours: counters are implausible",
                bytes_written / (1000 * GB)
            ),
            "app convention: over 200 GB/h sustained is implausible on a laptop",
        ));
    }
}

// ----------------------------------------------------------------- battery

fn battery_axis(scan: &Scan) -> AxisGrade {
    let mut findings = Vec::new();
    if scan.batteries.is_empty() {
        findings.push(finding(
            "battery",
            Verdict::NotGraded,
            Provenance::Measured,
            "No battery detected. On a laptop this means a missing or dead pack: check physically"
                .into(),
            "missing evidence is never a pass",
        ));
    }
    for b in &scan.batteries {
        battery_findings(b, &mut findings);
    }
    axis(Axis::Battery, findings)
}

fn battery_findings(b: &Battery, out: &mut Vec<Finding>) {
    let c = format!("battery/{}", b.name);
    match (b.design_capacity.get(), b.full_charge_capacity.get()) {
        (Some(d), Some(f)) if d.unit == f.unit && d.value > 0 => {
            if f.value == d.value {
                out.push(finding(
                    &c,
                    Verdict::NotGraded,
                    Provenance::Measured,
                    "Full-charge capacity exactly equals design capacity: treated as not reported"
                        .into(),
                    "an exact match is a firmware default, not a perfect battery",
                ));
            } else {
                let soh = f.value as f64 * 100.0 / d.value as f64;
                let (verdict, label, basis) = match soh {
                    s if s > 105.0 => (
                        Verdict::Amber,
                        "above design: fuel gauge reset or spoofed",
                        "app convention: >105% of design is not physically expected",
                    ),
                    s if s >= 90.0 => (Verdict::Green, "excellent", "app convention"),
                    s if s >= 80.0 => (
                        Verdict::Green,
                        "good",
                        "80% is Apple's end-of-rated-life line",
                    ),
                    s if s >= 70.0 => {
                        (Verdict::Amber, "fair", "below the 80% line; app convention")
                    }
                    _ => (Verdict::Red, "poor", "app convention"),
                };
                out.push(finding(
                    &c,
                    verdict,
                    Provenance::Measured,
                    format!("Holds {soh:.0}% of design capacity ({label})"),
                    basis,
                ));
            }
        }
        _ => out.push(finding(
            &c,
            Verdict::NotGraded,
            Provenance::Measured,
            "Capacity not reported".into(),
            "missing evidence is never a pass",
        )),
    }

    if let Some(&cycles) = b.cycle_count.get() {
        let (verdict, label) = match cycles {
            0..=299 => (Verdict::Green, "low"),
            300..=799 => (Verdict::Green, "moderate"),
            800..=1000 => (Verdict::Green, "approaching rated life"),
            _ => (Verdict::Amber, "beyond rated life"),
        };
        out.push(finding(
            &c,
            verdict,
            Provenance::Measured,
            format!("{cycles} charge cycles ({label})"),
            "Apple rates modern MacBooks at 1000 cycles; other OEMs publish no cycle rating",
        ));
    }
}

// ------------------------------------------------------------- encumbrance

/// How much a present encumbrance signal limits the buyer.
///
/// Red: someone else can lock, wipe, track or re-enrol the machine.
/// Amber: an ex-corporate marker or firmware-carried software that the seller
/// should explain, but that does not by itself stop the buyer using it.
/// Unknown signal ids are Red, so a new signal is never silently waved through.
fn encumbrance_severity(id: &str) -> Verdict {
    use crate::backend::signals::*;
    match id {
        ASSET_TAG | WPBT | DOMAIN_JOIN => Verdict::Amber,
        MDM_ENROLMENT | ENTRA_JOIN | AUTOPILOT_PROFILE | ABSOLUTE_AGENT => Verdict::Red,
        _ => Verdict::Red,
    }
}

fn encumbrance_axis(scan: &Scan) -> AxisGrade {
    let findings = scan
        .encumbrance
        .iter()
        .map(|s| {
            let verdict = if s.present {
                encumbrance_severity(&s.id)
            } else {
                Verdict::Green
            };
            finding(
                &format!("encumbrance/{}", s.id),
                verdict,
                s.provenance,
                s.detail.clone(),
                &s.source,
            )
        })
        .collect::<Vec<_>>();
    let mut grade = axis(Axis::Encumbrance, findings);
    // On Linux the strong signals (management enrolment, Autopilot, Absolute's
    // agent) are not visible, so a clean result here is not a cleared machine.
    if scan.os == "linux" && grade.verdict == Verdict::Green {
        grade.verdict = Verdict::NotGraded;
        grade.findings.push(finding(
            "encumbrance",
            Verdict::NotGraded,
            Provenance::NotAssessable,
            "Management enrolment, Autopilot profiles and installed tracking agents cannot be read from Linux".into(),
            "run the scan from the machine's installed Windows or macOS",
        ));
    }
    grade
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nvme(h: NvmeHealth) -> Drive {
        Drive {
            name: "nvme0n1".into(),
            transport: Transport::Nvme,
            rotational: Field::measured(Some(false), "test"),
            removable: false,
            model: Field::default(),
            serial: Field::default(),
            firmware: Field::default(),
            capacity_bytes: Field::default(),
            health: DriveHealth::Nvme(h),
        }
    }

    fn healthy_nvme() -> NvmeHealth {
        NvmeHealth {
            available_spare: 100,
            available_spare_threshold: 10,
            percentage_used: 3,
            power_on_hours: 2_000,
            data_units_written: 20_000_000, // ~10 TB
            ..NvmeHealth::default()
        }
    }

    fn battery(design: u64, full: u64, cycles: Option<u32>) -> Battery {
        let cap = |v| {
            Field::measured(
                Some(Capacity {
                    value: v,
                    unit: CapacityUnit::MilliwattHours,
                }),
                "test",
            )
        };
        Battery {
            name: "BAT0".into(),
            design_capacity: cap(design),
            full_charge_capacity: cap(full),
            cycle_count: Field::measured(cycles, "test"),
            ..Battery::default()
        }
    }

    fn scan(storage: Vec<Drive>, batteries: Vec<Battery>) -> Scan {
        Scan {
            os: "windows".into(),
            storage,
            batteries,
            ..Scan::default()
        }
    }

    fn axis_of(g: &Grade, a: Axis) -> &AxisGrade {
        g.axes.iter().find(|x| x.axis == a).unwrap()
    }

    #[test]
    fn headline_is_the_limiting_factor_not_an_average() {
        let mut bad = healthy_nvme();
        bad.media_errors = 1;
        let g = grade(&scan(
            vec![nvme(bad)],
            vec![battery(50_000, 48_000, Some(120))],
        ));
        assert_eq!(axis_of(&g, Axis::Battery).verdict, Verdict::Green);
        assert_eq!(g.headline, Verdict::Red);
        assert_eq!(g.limiting_axis, Some(Axis::Function));
    }

    #[test]
    fn healthy_machine_is_green_and_carries_the_storage_caveat() {
        let g = grade(&scan(
            vec![nvme(healthy_nvme())],
            vec![battery(50_000, 46_000, Some(300))],
        ));
        assert_eq!(g.headline, Verdict::Green);
        assert_eq!(g.limiting_axis, None);
        assert!(g.caveats.iter().any(|c| c.contains("36%")));
        assert!(g.caveats.iter().any(|c| c.contains("stolen")));
        assert!(g.not_assessable.iter().any(|n| n == "Theft status"));
        assert_eq!(g.grader_version, GRADER_VERSION);
    }

    #[test]
    fn not_reported_is_never_healthy() {
        let g = grade(&scan(vec![], vec![battery(50_000, 50_000, None)]));
        let b = axis_of(&g, Axis::Battery);
        assert_eq!(b.verdict, Verdict::NotGraded);
        assert!(!g.complete);
        assert_eq!(g.headline, Verdict::NotGraded);
    }

    #[test]
    fn battery_bands() {
        let v = |full| {
            axis_of(
                &grade(&scan(vec![], vec![battery(100, full, None)])),
                Axis::Battery,
            )
            .verdict
        };
        assert_eq!(v(95), Verdict::Green);
        assert_eq!(v(80), Verdict::Green);
        assert_eq!(v(79), Verdict::Amber);
        assert_eq!(v(69), Verdict::Red);
        assert_eq!(v(110), Verdict::Amber);
        let cycles = grade(&scan(vec![], vec![battery(100, 90, Some(1200))]));
        assert_eq!(axis_of(&cycles, Axis::Battery).verdict, Verdict::Amber);
    }

    #[test]
    fn ata_pending_sectors_are_red_and_reallocations_amber() {
        let ata = |attrs: &[(u8, u64)]| Drive {
            name: "sda".into(),
            transport: Transport::Sata,
            health: DriveHealth::Ata(AtaHealth {
                attributes: attrs
                    .iter()
                    .map(|&(id, raw)| AtaAttribute {
                        id,
                        raw,
                        current: 100,
                        worst: 100,
                    })
                    .collect(),
            }),
            ..nvme(NvmeHealth::default())
        };
        let f = |attrs| axis_of(&grade(&scan(vec![ata(attrs)], vec![])), Axis::Function).verdict;
        assert_eq!(f(&[(5, 0), (197, 2)]), Verdict::Red);
        assert_eq!(f(&[(5, 8), (197, 0)]), Verdict::Amber);
        assert_eq!(f(&[(5, 0), (187, 0), (197, 0)]), Verdict::Green);
        assert_eq!(f(&[(9, 100)]), Verdict::NotGraded);
    }

    #[test]
    fn usb_drives_are_never_graded_as_internal() {
        let mut d = nvme(healthy_nvme());
        d.transport = Transport::Usb;
        let g = grade(&scan(vec![d], vec![]));
        assert_eq!(axis_of(&g, Axis::Function).verdict, Verdict::NotGraded);
    }

    #[test]
    fn implausible_write_counters_are_flagged() {
        let mut h = healthy_nvme();
        h.power_on_hours = 40_000;
        h.data_units_written = 4_000_000; // ~2 TB
        let g = grade(&scan(vec![nvme(h)], vec![]));
        let f = axis_of(&g, Axis::Function);
        assert_eq!(f.verdict, Verdict::Amber);
        assert!(f
            .findings
            .iter()
            .any(|x| x.provenance == Provenance::Inferred));
    }

    #[test]
    fn nvme_wear_and_spare() {
        let mut h = healthy_nvme();
        h.percentage_used = 85;
        assert_eq!(
            axis_of(&grade(&scan(vec![nvme(h.clone())], vec![])), Axis::Function).verdict,
            Verdict::Amber
        );
        h.available_spare = 9;
        assert_eq!(
            axis_of(&grade(&scan(vec![nvme(h)], vec![])), Axis::Function).verdict,
            Verdict::Red
        );
    }

    #[test]
    fn clean_linux_encumbrance_is_not_a_clearance() {
        let mut s = scan(vec![], vec![]);
        s.os = "linux".into();
        s.encumbrance.push(EncumbranceSignal {
            id: "smbios_asset_tag".into(),
            present: false,
            provenance: Provenance::Claimed,
            source: "test".into(),
            detail: "no tag".into(),
        });
        assert_eq!(
            axis_of(&grade(&s), Axis::Encumbrance).verdict,
            Verdict::NotGraded
        );
        s.encumbrance[0].present = true;
        assert_eq!(
            axis_of(&grade(&s), Axis::Encumbrance).verdict,
            Verdict::Amber
        );
    }

    fn signal(id: &str, present: bool) -> EncumbranceSignal {
        EncumbranceSignal {
            id: id.into(),
            present,
            provenance: Provenance::Claimed,
            source: "test".into(),
            detail: id.into(),
        }
    }

    #[test]
    fn management_is_red_and_markers_amber() {
        use crate::backend::signals::*;
        let v = |sigs: Vec<EncumbranceSignal>| {
            let mut s = scan(vec![], vec![]);
            s.encumbrance = sigs;
            axis_of(&grade(&s), Axis::Encumbrance).verdict
        };
        let clean = || {
            [
                ASSET_TAG,
                WPBT,
                MDM_ENROLMENT,
                ENTRA_JOIN,
                DOMAIN_JOIN,
                AUTOPILOT_PROFILE,
                ABSOLUTE_AGENT,
            ]
            .map(|id| signal(id, false))
            .to_vec()
        };
        assert_eq!(v(clean()), Verdict::Green);
        for (id, expected) in [
            (ASSET_TAG, Verdict::Amber),
            (WPBT, Verdict::Amber),
            (DOMAIN_JOIN, Verdict::Amber),
            (MDM_ENROLMENT, Verdict::Red),
            (ENTRA_JOIN, Verdict::Red),
            (AUTOPILOT_PROFILE, Verdict::Red),
            (ABSOLUTE_AGENT, Verdict::Red),
            ("some_future_signal", Verdict::Red),
        ] {
            let mut sigs = clean();
            sigs.push(signal(id, true));
            assert_eq!(v(sigs), expected, "{id}");
        }
    }

    #[test]
    fn clean_windows_still_lists_what_it_cannot_see() {
        let g = grade(&scan(vec![], vec![]));
        assert!(g.not_assessable.iter().any(|n| n.contains("Autopilot")));
        assert!(g
            .not_assessable
            .iter()
            .any(|n| n.contains("supervisor password")));
    }
}
