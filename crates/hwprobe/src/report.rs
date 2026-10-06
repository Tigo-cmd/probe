//! Plain-text rendering of a scan and its grade.
//!
//! The wording rules here are part of the product, not presentation:
//! every value is tagged with its epistemic class, missing values say
//! "not reported", and clean results state their coverage.

use std::fmt::Write;

use crate::grade::{Axis, Grade, Verdict};
use crate::model::*;

pub fn render(scan: &Scan, grade: &Grade) -> String {
    let mut o = String::new();
    let _ = render_into(&mut o, scan, grade);
    o
}

fn render_into(o: &mut String, scan: &Scan, grade: &Grade) -> std::fmt::Result {
    writeln!(
        o,
        "hwprobe {} · grader {} · {}",
        scan.tool_version, grade.grader_version, scan.os
    )?;
    if !scan.elevated {
        writeln!(
            o,
            "Not elevated: run as administrator/root for serials and drive health."
        )?;
    }
    writeln!(o)?;

    let completeness = if grade.complete {
        ""
    } else {
        " (incomplete: some axes not graded)"
    };
    writeln!(
        o,
        "OVERALL  {}{completeness}",
        verdict_label(grade.headline)
    )?;
    if let Some(axis) = grade.limiting_axis {
        writeln!(o, "Limited by: {}", axis_label(axis))?;
    }
    for a in &grade.axes {
        let summary = match a.verdict {
            Verdict::Green => format!("no faults detected in {} checks", a.checks),
            Verdict::NotGraded => "not graded".to_string(),
            _ => format!("{} checks", a.checks),
        };
        writeln!(
            o,
            "  {:<12} {:<11} {summary}",
            axis_label(a.axis),
            verdict_label(a.verdict)
        )?;
    }

    for a in &grade.axes {
        writeln!(o, "\n{}", axis_label(a.axis).to_uppercase())?;
        for f in &a.findings {
            writeln!(
                o,
                "  {:<11} {} {}",
                verdict_label(f.verdict),
                tag(f.provenance),
                f.message
            )?;
            writeln!(o, "              basis: {}", f.basis)?;
        }
    }

    writeln!(o, "\nIDENTITY")?;
    let id = &scan.identity;
    field(o, "Vendor", &id.vendor)?;
    field(o, "Model", &id.model)?;
    field(o, "Serial", &id.serial)?;
    field(o, "Board serial", &id.board_serial)?;
    field(o, "UUID", &id.uuid)?;
    field(o, "BIOS", &id.bios_version)?;
    field(o, "CPU", &scan.cpu.brand)?;
    field(o, "Cores", &scan.cpu.physical_cores)?;
    field(o, "Threads", &scan.cpu.logical_cores)?;
    let mem = &scan.memory.total_bytes;
    field(
        o,
        "Usable RAM",
        &mem.map(|b| format!("{:.1} GiB", *b as f64 / (1u64 << 30) as f64)),
    )?;

    for d in &scan.storage {
        writeln!(o, "\nSTORAGE {} ({:?})", d.name, d.transport)?;
        field(o, "Model", &d.model)?;
        field(o, "Serial", &d.serial)?;
        field(o, "Firmware", &d.firmware)?;
        field(
            o,
            "Capacity",
            &d.capacity_bytes
                .map(|b| format!("{:.0} GB", *b as f64 / 1e9)),
        )?;
    }

    for b in &scan.batteries {
        writeln!(o, "\nBATTERY {}", b.name)?;
        field(o, "Manufacturer", &b.manufacturer)?;
        let cap = |c: &Field<Capacity>| {
            c.map(|c| {
                format!(
                    "{} {}",
                    c.value,
                    if c.unit == CapacityUnit::MilliwattHours {
                        "mWh"
                    } else {
                        "mAh"
                    }
                )
            })
        };
        field(o, "Design", &cap(&b.design_capacity))?;
        field(o, "Full charge", &cap(&b.full_charge_capacity))?;
        field(o, "Cycles", &b.cycle_count)?;
    }

    for d in &scan.displays {
        let res = match (d.native_width, d.native_height) {
            (Some(w), Some(h)) => format!("{w}×{h}"),
            _ => "resolution not reported".into(),
        };
        let made = d
            .manufacture_year
            .map(|y| format!(", made {y}"))
            .unwrap_or_default();
        writeln!(
            o,
            "\nDISPLAY {}  {} {} {}{made} [CLAIMED, EDID]",
            d.connector,
            d.manufacturer_id.as_deref().unwrap_or("?"),
            d.name.as_deref().unwrap_or(""),
            res
        )?;
    }

    writeln!(o, "\nNOT ASSESSABLE BY SOFTWARE")?;
    for n in &grade.not_assessable {
        writeln!(o, "  · {n}")?;
    }

    writeln!(o, "\nREAD THIS")?;
    for c in &grade.caveats {
        writeln!(o, "  · {c}")?;
    }

    if !scan.probe_notes.is_empty() {
        writeln!(o, "\nPROBE NOTES")?;
        for n in &scan.probe_notes {
            writeln!(o, "  · {}: {}", n.component, n.message)?;
        }
    }
    Ok(())
}

fn field<T: std::fmt::Display>(o: &mut String, label: &str, f: &Field<T>) -> std::fmt::Result {
    match (&f.value, &f.note) {
        (Some(v), _) => writeln!(o, "  {label:<13} {v} {}", tag(f.provenance)),
        (None, Some(n)) => writeln!(o, "  {label:<13} not reported ({n})"),
        (None, None) => writeln!(o, "  {label:<13} not reported"),
    }
}

pub fn verdict_label(v: Verdict) -> &'static str {
    match v {
        Verdict::Green => "GREEN",
        Verdict::Amber => "AMBER",
        Verdict::Red => "RED",
        Verdict::NotGraded => "NOT GRADED",
    }
}

pub fn axis_label(a: Axis) -> &'static str {
    match a {
        Axis::Function => "Function",
        Axis::Battery => "Battery",
        Axis::Encumbrance => "Encumbrance",
    }
}

pub fn tag(p: Provenance) -> &'static str {
    match p {
        Provenance::Measured => "[MEASURED]",
        Provenance::Claimed => "[CLAIMED]",
        Provenance::Inferred => "[INFERRED]",
        Provenance::NotAssessable => "[NOT ASSESSABLE]",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_never_says_healthy_or_not_stolen() {
        let scan = Scan {
            os: "linux".into(),
            ..Scan::default()
        };
        let text = render(&scan, &crate::grade::grade(&scan));
        let lower = text.to_lowercase();
        assert!(!lower.contains("healthy"));
        assert!(!lower.contains("not stolen"));
        assert!(text.contains("NOT GRADED"));
        assert!(text.contains("Theft status"));
    }
}
