//! A scan and its grade as one self-contained HTML page.
//!
//! Same content and wording rules as the plain-text report in
//! [`crate::report`], laid out for people: a score ring, coloured verdicts
//! and cards. No scripts and no outside files, so the page opens offline in
//! any browser, can be emailed as one file, and prints cleanly.

use std::fmt::Write;

use crate::assess::{self, AreaScore, Band, Fit, Tone};
use crate::grade::{Axis, Grade, Verdict};
use crate::handson::{kind_label, HandsOn, Outcome};
use crate::model::*;
use crate::report::use_label;
use crate::windows11::Readiness;

/// The full page, including hands-on test results when any were run.
pub fn render_html(scan: &Scan, grade: &Grade, hands_on: Option<&HandsOn>) -> String {
    let mut o = String::with_capacity(32 * 1024);
    let _ = page(&mut o, scan, grade, hands_on);
    o
}

fn page(
    o: &mut String,
    scan: &Scan,
    grade: &Grade,
    hands_on: Option<&HandsOn>,
) -> std::fmt::Result {
    let a = assess::assess_with(scan, grade, hands_on);
    let name = machine_name(&scan.identity);
    let date = date_of(scan.started_at);

    write!(
        o,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>probe report · {} · {}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<main>\n",
        esc(&name),
        esc(&date)
    )?;

    // ------------------------------------------------------------ header
    writeln!(
        o,
        "<header class=\"top\">{MARK}<div><p class=\"eyebrow\">Condition report</p>\
         <h1>{}</h1><p class=\"muted\">Scanned {} · {} · probe {}</p></div></header>",
        esc(&name),
        esc(&date),
        esc(&os_label(&scan.os)),
        esc(&scan.tool_version)
    )?;
    if !scan.elevated {
        o.push_str(
            "<p class=\"notice\">This scan ran without administrator rights, so drive health, \
             serial numbers and some locks may be missing. Run probe again with administrator \
             rights for a complete report.</p>\n",
        );
    }

    // -------------------------------------------------------- score card
    let c = &a.condition;
    let tone = verdict_tone(grade.headline);
    o.push_str("<section class=\"card hero\">");
    ring(o, c.score, tone)?;
    write!(
        o,
        "<div class=\"hero-text\"><span class=\"pill {tone}\">{}</span><h2>{}</h2>",
        verdict_headline(grade.headline),
        c.band.map(band_label).unwrap_or("Not scored")
    )?;
    let mut sub = Vec::new();
    if c.scored_areas < c.areas.len() {
        sub.push(format!(
            "Scored from {} of {} areas; the rest could not be read.",
            c.scored_areas,
            c.areas.len()
        ));
    }
    if let Some(cap) = c.cap {
        sub.push(format!("Held to {cap} by the most serious finding."));
    }
    if !grade.complete {
        sub.push("Some checks could not run, so this result is incomplete.".into());
    }
    if !sub.is_empty() {
        write!(o, "<p class=\"muted\">{}</p>", esc(&sub.join(" ")))?;
    }
    o.push_str("<div class=\"areas\">");
    for area in &c.areas {
        area_row(o, area)?;
    }
    o.push_str("</div></div></section>\n");

    // ------------------------------------------------------- suited for
    o.push_str(
        "<section><h2 class=\"section\">What it is good for</h2>\
         <p class=\"muted small\">Estimates. Processor and graphics are rated from their model names.</p>\
         <div class=\"grid uses\">",
    );
    for u in &a.uses {
        let t = fit_tone(u.fit);
        write!(
            o,
            "<div class=\"card use\"><div class=\"use-head\"><h3>{}</h3><span class=\"pill {t}\">{}</span></div>",
            use_label(u.use_case),
            fit_label(u.fit)
        )?;
        bar(o, u.score, t)?;
        if let Some(r) = u.reasons.first() {
            write!(
                o,
                "<p class=\"small\"><span class=\"dot {}\"></span>{} {}</p>",
                reason_tone(r.tone),
                esc(&r.text),
                prov(r.provenance)
            )?;
        }
        o.push_str("</div>");
    }
    o.push_str("</div></section>\n");

    // -------------------------------------------------------- windows 11
    let w = &a.windows11;
    if w.readiness != Readiness::NotApplicable {
        let (tone, label) = match w.readiness {
            Readiness::Ready => ("green", "Ready"),
            Readiness::Fixable => ("amber", "Fixable"),
            Readiness::NotSupported => ("red", "Not supported"),
            _ => ("grey", "Not known"),
        };
        write!(
            o,
            "<section><h2 class=\"section\">Windows 11</h2><div class=\"card list\">\
             <div class=\"list-head\"><p>{}</p><span class=\"pill {tone}\">{label}</span></div>",
            esc(&w.summary)
        )?;
        for r in &w.requirements {
            let t = match r.met {
                Some(true) => "green",
                Some(false) => "red",
                None => "grey",
            };
            write!(
                o,
                "<div class=\"row finding\"><span class=\"dot {t}\"></span><div><p><b>{}</b>: {} {}</p></div></div>",
                esc(&r.label),
                esc(&r.detail),
                prov(r.provenance)
            )?;
        }
        if !w.fixes.is_empty() {
            let title = if w.readiness == Readiness::Fixable {
                "How to fix it"
            } else {
                "Worth doing"
            };
            write!(
                o,
                "<div class=\"row\"><div><p><b>{title}</b></p><ul class=\"fixes\">"
            )?;
            for f in &w.fixes {
                write!(o, "<li>{}</li>", esc(f))?;
            }
            o.push_str("</ul></div></div>");
        }
        o.push_str("</div></section>\n");
    }

    // ------------------------------------------------------- next steps
    if !a.tips.is_empty() {
        o.push_str("<section><h2 class=\"section\">Next steps</h2><ol class=\"card steps\">");
        for t in &a.tips {
            write!(o, "<li>{}</li>", esc(t))?;
        }
        o.push_str("</ol></section>\n");
    }

    // --------------------------------------------------------- hands-on
    if let Some(h) = hands_on.filter(|h| !h.tests.is_empty()) {
        o.push_str(
            "<section><h2 class=\"section\">Hands-on tests</h2>\
             <p class=\"muted small\">Observed by the person who ran probe.</p><div class=\"card list\">",
        );
        for t in &h.tests {
            let (tone, label) = match t.outcome {
                Outcome::Passed => ("green", "Passed"),
                Outcome::Problems => ("red", "Problems"),
                Outcome::Incomplete => ("grey", "Not finished"),
            };
            write!(
                o,
                "<div class=\"row\"><div><h3>{}</h3><p class=\"muted small\">{}</p>",
                esc(kind_label(t.kind)),
                esc(&t.summary)
            )?;
            if !t.problems.is_empty() {
                o.push_str("<ul class=\"problems\">");
                for p in &t.problems {
                    write!(o, "<li>{}</li>", esc(p))?;
                }
                o.push_str("</ul>");
            }
            write!(o, "</div><span class=\"pill {tone}\">{label}</span></div>")?;
        }
        o.push_str("</div></section>\n");
    }

    // --------------------------------------------------------- findings
    o.push_str("<section><h2 class=\"section\">Findings</h2>");
    for axis in &grade.axes {
        write!(
            o,
            "<div class=\"card list\"><div class=\"list-head\"><h3>{}</h3><span class=\"pill {}\">{}</span></div>",
            area_label(axis.axis),
            verdict_tone(axis.verdict),
            verdict_short(axis.verdict)
        )?;
        if axis.findings.is_empty() {
            o.push_str("<p class=\"muted small row\">Nothing to report.</p>");
        }
        for f in &axis.findings {
            write!(
                o,
                "<div class=\"row finding\"><span class=\"dot {}\"></span><div><p>{} {}</p>\
                 <p class=\"faint small\">Basis: {}</p></div></div>",
                verdict_tone(f.verdict),
                esc(&f.message),
                prov(f.provenance),
                esc(&f.basis)
            )?;
        }
        o.push_str("</div>");
    }
    o.push_str("</section>\n");

    // ------------------------------------------------------------ specs
    o.push_str("<section><h2 class=\"section\">About this computer</h2><div class=\"grid specs\">");
    let id = &scan.identity;
    let mem = scan
        .memory
        .total_bytes
        .map(|b| format!("{:.1} GiB usable", *b as f64 / (1u64 << 30) as f64));
    table(
        o,
        "System",
        &[
            row("Maker", &id.vendor),
            row("Model", &id.model),
            row("Serial number", &id.serial),
            row("Board serial", &id.board_serial),
            row("BIOS version", &id.bios_version),
            row("Processor", &scan.cpu.brand),
            row("Cores", &scan.cpu.physical_cores),
            row("Threads", &scan.cpu.logical_cores),
            row("Memory", &mem),
        ],
    )?;
    for d in &scan.storage {
        let cap = d
            .capacity_bytes
            .map(|b| format!("{:.0} GB", *b as f64 / 1e9));
        table(
            o,
            &format!("Drive {} ({:?})", d.name, d.transport),
            &[
                row("Model", &d.model),
                row("Serial number", &d.serial),
                row("Firmware", &d.firmware),
                row("Capacity", &cap),
            ],
        )?;
    }
    for b in &scan.batteries {
        let cap = |c: &Field<Capacity>| {
            c.map(|c| {
                let unit = if c.unit == CapacityUnit::MilliwattHours {
                    "mWh"
                } else {
                    "mAh"
                };
                format!("{} {unit}", c.value)
            })
        };
        table(
            o,
            &format!("Battery {}", b.name),
            &[
                row("Maker", &b.manufacturer),
                row("Original capacity", &cap(&b.design_capacity)),
                row("Capacity now", &cap(&b.full_charge_capacity)),
                row("Charge cycles", &b.cycle_count),
            ],
        )?;
    }
    for g in &scan.gpus {
        let vram = g
            .vram_bytes
            .map(|b| format!("{:.1} GB", *b as f64 / (1u64 << 30) as f64));
        table(
            o,
            "Graphics",
            &[row("Name", &g.name), row("Video memory", &vram)],
        )?;
    }
    for d in &scan.displays {
        let res = match (d.native_width, d.native_height) {
            (Some(w), Some(h)) => format!("{w} × {h}"),
            _ => "not reported".into(),
        };
        let name = [d.manufacturer_id.as_deref(), d.name.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let mut rows = vec![
            plain(
                "Name",
                if name.is_empty() {
                    "not reported".into()
                } else {
                    name
                },
            ),
            plain("Resolution", res),
        ];
        if let Some(y) = d.manufacture_year {
            rows.push(plain("Made", y.to_string()));
        }
        rows.push(plain("Source", "Claimed by the screen (EDID)".into()));
        table(o, &format!("Screen {}", d.connector), &rows)?;
    }
    o.push_str("</div></section>\n");

    // ------------------------------------------------------------ limits
    o.push_str("<section class=\"grid two\"><div class=\"card callout\"><h3>Software cannot check</h3><ul>");
    for n in &grade.not_assessable {
        write!(o, "<li>{}</li>", esc(n))?;
    }
    o.push_str("</ul></div><div class=\"card callout warn\"><h3>Read this</h3><ul>");
    for c in &grade.caveats {
        write!(o, "<li>{}</li>", esc(c))?;
    }
    o.push_str("</ul></div></section>\n");

    if !scan.probe_notes.is_empty() {
        o.push_str(
            "<details class=\"card notes\"><summary>Technical notes from the scan</summary><ul>",
        );
        for n in &scan.probe_notes {
            write!(
                o,
                "<li><b>{}</b>: {}</li>",
                esc(&n.component),
                esc(&n.message)
            )?;
        }
        o.push_str("</ul></details>\n");
    }

    writeln!(
        o,
        "<footer><p><b>Measured</b> values were read from a sensor or controller. <b>Claimed</b> \
         values come from rewritable firmware and can be altered. <b>Inferred</b> values are \
         estimates.</p><p>Made with probe {} (grader {}) · \
         <a href=\"https://probe-pc.github.io\">probe-pc.github.io</a></p></footer>",
        esc(&scan.tool_version),
        esc(&grade.grader_version)
    )?;
    o.push_str("</main>\n</body>\n</html>\n");
    Ok(())
}

// ------------------------------------------------------------------ parts

fn ring(o: &mut String, score: Option<u8>, tone: &str) -> std::fmt::Result {
    let value = score.unwrap_or(0) as f64;
    // Circumference of r=52 is about 326.7.
    let len = 326.7;
    let filled = len * value / 100.0;
    write!(
        o,
        "<div class=\"ring\"><svg viewBox=\"0 0 120 120\" aria-hidden=\"true\">\
         <circle cx=\"60\" cy=\"60\" r=\"52\" class=\"track\"/>\
         <circle cx=\"60\" cy=\"60\" r=\"52\" class=\"arc {tone}\" stroke-dasharray=\"{filled:.1} {len}\" \
         transform=\"rotate(-90 60 60)\"/></svg><div><b>{}</b><span>out of 100</span></div></div>",
        score.map(|s| s.to_string()).unwrap_or_else(|| "–".into())
    )
}

fn bar(o: &mut String, score: Option<u8>, tone: &str) -> std::fmt::Result {
    match score {
        Some(s) => write!(
            o,
            "<div class=\"bar\"><i class=\"{tone}\" style=\"width:{s}%\"></i></div>"
        ),
        None => write!(o, "<div class=\"bar\"></div>"),
    }
}

fn area_row(o: &mut String, a: &AreaScore) -> std::fmt::Result {
    let tone = verdict_tone(a.verdict);
    write!(
        o,
        "<div class=\"area\"><div class=\"area-head\"><span>{}</span><b>{}</b></div>",
        // The area score covers the drives only; devices show under findings.
        match a.axis {
            Axis::Function => "Drives",
            other => area_label(other),
        },
        a.score.map(|s| s.to_string()).unwrap_or_else(|| "–".into())
    )?;
    bar(o, a.score, tone)?;
    write!(o, "<p class=\"muted small\">{}</p></div>", esc(&a.summary))
}

struct Row {
    label: &'static str,
    value: String,
    tag: &'static str,
}

fn row<T: std::fmt::Display>(label: &'static str, f: &Field<T>) -> Row {
    match (&f.value, &f.note) {
        (Some(v), _) => Row {
            label,
            value: v.to_string(),
            tag: prov(f.provenance),
        },
        (None, Some(n)) => Row {
            label,
            value: format!("not reported ({n})"),
            tag: "",
        },
        (None, None) => Row {
            label,
            value: "not reported".into(),
            tag: "",
        },
    }
}

fn plain(label: &'static str, value: String) -> Row {
    Row {
        label,
        value,
        tag: "",
    }
}

fn table(o: &mut String, title: &str, rows: &[Row]) -> std::fmt::Result {
    write!(o, "<div class=\"card\"><h3>{}</h3><dl>", esc(title))?;
    for r in rows {
        let class = if r.value.starts_with("not reported") {
            " class=\"faint\""
        } else {
            ""
        };
        write!(
            o,
            "<div><dt>{}</dt><dd{class}>{} {}</dd></div>",
            r.label,
            esc(&r.value),
            r.tag
        )?;
    }
    o.push_str("</dl></div>");
    Ok(())
}

// ----------------------------------------------------------------- labels

fn machine_name(id: &Identity) -> String {
    let vendor = id.vendor.get().map(|s| s.trim()).unwrap_or("");
    let model = id.model.get().map(|s| s.trim()).unwrap_or("");
    let name = if model.to_lowercase().starts_with(&vendor.to_lowercase()) {
        model.to_string()
    } else {
        format!("{vendor} {model}").trim().to_string()
    };
    if name.is_empty() {
        "Unidentified computer".into()
    } else {
        name
    }
}

fn os_label(os: &str) -> String {
    match os {
        "windows" => "Windows".into(),
        "macos" => "macOS".into(),
        "linux" => "Linux".into(),
        other => other.into(),
    }
}

fn area_label(a: Axis) -> &'static str {
    match a {
        Axis::Function => "Drives and devices",
        Axis::Battery => "Battery",
        Axis::Encumbrance => "Ownership",
    }
}

fn verdict_tone(v: Verdict) -> &'static str {
    match v {
        Verdict::Green => "green",
        Verdict::Amber => "amber",
        Verdict::Red => "red",
        Verdict::NotGraded => "grey",
    }
}

fn verdict_headline(v: Verdict) -> &'static str {
    match v {
        Verdict::Red => "Problems found",
        Verdict::Amber => "Needs attention",
        Verdict::Green => "No faults detected",
        Verdict::NotGraded => "Not graded: evidence missing",
    }
}

fn verdict_short(v: Verdict) -> &'static str {
    match v {
        Verdict::Red => "Problem",
        Verdict::Amber => "Attention",
        Verdict::Green => "No faults detected",
        Verdict::NotGraded => "Not graded",
    }
}

fn band_label(b: Band) -> &'static str {
    match b {
        Band::Excellent => "Excellent condition",
        Band::Good => "Good condition",
        Band::Fair => "Fair condition",
        Band::Poor => "Poor condition",
    }
}

fn fit_label(f: Fit) -> &'static str {
    match f {
        Fit::Great => "Great fit",
        Fit::Good => "Good fit",
        Fit::Limited => "Limited",
        Fit::Poor => "Poor fit",
        Fit::Blocked => "Blocked",
        Fit::Unknown => "Unknown",
    }
}

fn fit_tone(f: Fit) -> &'static str {
    match f {
        Fit::Great | Fit::Good => "green",
        Fit::Limited => "amber",
        Fit::Poor | Fit::Blocked => "red",
        Fit::Unknown => "grey",
    }
}

fn reason_tone(t: Tone) -> &'static str {
    match t {
        Tone::Good => "green",
        Tone::Fair => "amber",
        Tone::Bad => "red",
    }
}

fn prov(p: Provenance) -> &'static str {
    match p {
        Provenance::Measured => "<span class=\"tag measured\">Measured</span>",
        Provenance::Claimed => "<span class=\"tag\">Claimed</span>",
        Provenance::Inferred => "<span class=\"tag\">Inferred</span>",
        Provenance::NotAssessable => "<span class=\"tag\">Not assessable</span>",
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// "27 October 2026" in UTC, from Unix seconds.
fn date_of(secs: u64) -> String {
    if secs == 0 {
        return "on an unknown date".into();
    }
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}

const MARK: &str = "<svg class=\"mark\" viewBox=\"0 0 120 120\" aria-hidden=\"true\">\
<rect width=\"120\" height=\"120\" rx=\"27\" fill=\"#1c1c1e\"/>\
<g fill=\"none\" stroke-linecap=\"round\" stroke-width=\"9\" transform=\"rotate(-90 60 60)\">\
<circle cx=\"60\" cy=\"60\" r=\"38\" stroke=\"#34c759\" stroke-dasharray=\"207 239\"/>\
<circle cx=\"60\" cy=\"60\" r=\"26\" stroke=\"#0a84ff\" stroke-dasharray=\"123 163\"/>\
<circle cx=\"60\" cy=\"60\" r=\"14\" stroke=\"#ff9f0a\" stroke-dasharray=\"58 88\"/></g></svg>";

const CSS: &str = r#"
:root{--bg:#f5f5f7;--card:#fff;--text:#1d1d1f;--muted:#6e6e73;--faint:#8e8e93;--line:rgba(0,0,0,.08);
--green:#248a3d;--green-bg:#e3f5e8;--amber:#b25000;--amber-bg:#fff1dc;--red:#d70015;--red-bg:#ffe5e7;
--grey:#6e6e73;--grey-bg:#ececf0;--blue:#0071e3;color-scheme:light dark}
@media (prefers-color-scheme:dark){:root{--bg:#000;--card:#1c1c1e;--text:#f5f5f7;--muted:#a1a1a6;--faint:#8e8e93;
--line:rgba(255,255,255,.1);--green:#30d158;--green-bg:rgba(48,209,88,.16);--amber:#ff9f0a;--amber-bg:rgba(255,159,10,.16);
--red:#ff453a;--red-bg:rgba(255,69,58,.16);--grey:#a1a1a6;--grey-bg:rgba(255,255,255,.08);--blue:#0a84ff}}
*{box-sizing:border-box;margin:0}
body{background:var(--bg);color:var(--text);font:15px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Inter,Roboto,Helvetica,Arial,sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:920px;margin:0 auto;padding:40px 20px 56px;display:grid;grid-template-columns:minmax(0,1fr);gap:28px}
p,li,dd,h1{overflow-wrap:anywhere}
h1{font-size:30px;line-height:1.15;letter-spacing:-.02em}
h2{font-size:24px;letter-spacing:-.015em}
h3{font-size:16px;font-weight:600}
.section{font-size:20px;margin-bottom:4px}
.muted{color:var(--muted)}.faint{color:var(--faint)}.small{font-size:13px}
.eyebrow{font-size:13px;font-weight:600;color:var(--muted);text-transform:uppercase;letter-spacing:.06em}
.top{display:flex;gap:18px;align-items:center}.mark{width:60px;height:60px;flex:none}
.notice{background:var(--amber-bg);color:var(--amber);padding:12px 16px;border-radius:14px;font-size:14px}
.card{background:var(--card);border-radius:20px;padding:22px;box-shadow:0 1px 2px rgba(0,0,0,.04),0 0 0 1px var(--line)}
section>.card+.card{margin-top:14px}
.hero{display:grid;grid-template-columns:180px 1fr;gap:28px;align-items:center;padding:28px}
.hero-text{display:grid;gap:10px;justify-items:start}
.ring{position:relative;width:180px;height:180px}.ring svg{width:100%;height:100%}
.ring circle{fill:none;stroke-width:12;stroke-linecap:round}.track{stroke:var(--grey-bg)}
.arc.green{stroke:var(--green)}.arc.amber{stroke:var(--amber)}.arc.red{stroke:var(--red)}.arc.grey{stroke:var(--grey)}
.ring div{position:absolute;inset:0;display:grid;place-content:center;text-align:center}
.ring b{font-size:48px;line-height:1;letter-spacing:-.03em}.ring span{font-size:12px;color:var(--muted)}
.pill{display:inline-block;padding:3px 10px;border-radius:999px;font-size:12px;font-weight:600;white-space:nowrap}
.pill.green{background:var(--green-bg);color:var(--green)}.pill.amber{background:var(--amber-bg);color:var(--amber)}
.pill.red{background:var(--red-bg);color:var(--red)}.pill.grey{background:var(--grey-bg);color:var(--grey)}
.areas{display:grid;gap:14px;width:100%;margin-top:6px}
.area-head{display:flex;justify-content:space-between;font-weight:600;font-size:14px}
.bar{height:8px;border-radius:99px;background:var(--grey-bg);overflow:hidden;margin:6px 0}
.bar i{display:block;height:100%;border-radius:99px}
.bar .green{background:var(--green)}.bar .amber{background:var(--amber)}.bar .red{background:var(--red)}.bar .grey{background:var(--grey)}
.grid{display:grid;gap:14px;margin-top:12px}
.uses{grid-template-columns:repeat(auto-fill,minmax(min(260px,100%),1fr))}
.specs{grid-template-columns:repeat(auto-fill,minmax(min(400px,100%),1fr))}
.two{grid-template-columns:1fr 1fr;margin:0}
.use{display:grid;gap:6px;padding:18px}
.use-head{display:flex;justify-content:space-between;gap:10px;align-items:center}
.dot{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:8px;flex:none;vertical-align:1px}
.dot.green{background:var(--green)}.dot.amber{background:var(--amber)}.dot.red{background:var(--red)}.dot.grey{background:var(--grey)}
.steps{padding:20px 22px 20px 44px;display:grid;gap:10px;margin-top:12px}
.list{padding:6px 22px;margin-top:12px}
.list-head{display:flex;justify-content:space-between;align-items:center;padding:14px 0}
.row{display:flex;justify-content:space-between;gap:16px;align-items:flex-start;padding:14px 0;border-top:1px solid var(--line)}
.list>.row:first-child{border-top:0}
.finding{justify-content:flex-start}.finding .dot{margin-top:8px}
.fixes{margin-top:6px;padding-left:18px;font-size:14px;color:var(--muted)}
.problems{margin-top:6px;padding-left:18px;color:var(--red);font-size:13px}
.tag{display:inline-block;font-size:11px;font-weight:600;padding:1px 7px;border-radius:6px;background:var(--grey-bg);color:var(--muted);vertical-align:1px;margin-left:4px}
.tag.measured{background:var(--green-bg);color:var(--green)}
dl{display:grid;margin-top:10px}
dl div{display:grid;grid-template-columns:140px 1fr;gap:12px;padding:8px 0;border-top:1px solid var(--line);font-size:14px}
dt{color:var(--muted)}dd{word-break:break-word}
.callout ul{margin-top:10px;padding-left:18px;display:grid;gap:6px;font-size:14px;color:var(--muted)}
.callout.warn{background:var(--amber-bg);box-shadow:none}.callout.warn h3{color:var(--amber)}
.notes summary{cursor:pointer;font-weight:600}.notes ul{margin-top:10px;padding-left:18px;font-size:13px;color:var(--muted);display:grid;gap:4px}
footer{font-size:12px;color:var(--faint);display:grid;gap:6px;border-top:1px solid var(--line);padding-top:18px}
footer a{color:var(--blue)}
@media (max-width:640px){.hero{grid-template-columns:1fr;justify-items:center;text-align:center}.hero-text{justify-items:center}
.two,.specs{grid-template-columns:1fr}dl div{grid-template-columns:110px 1fr}h1{font-size:24px}}
@media print{body{background:#fff}main{padding:0;gap:18px}.card{box-shadow:0 0 0 1px #ddd;break-inside:avoid}
details{display:block}details>*{display:block}a{color:inherit}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handson::{TestKind, TestResult};

    fn sample() -> Scan {
        Scan {
            os: "windows".into(),
            tool_version: "0.1.0".into(),
            started_at: 1_793_000_000,
            elevated: true,
            identity: Identity {
                vendor: Field::claimed(Some("Dell Inc.".into()), "test"),
                model: Field::claimed(Some("Latitude <7420>".into()), "test"),
                ..Identity::default()
            },
            ..Scan::default()
        }
    }

    #[test]
    fn page_is_complete_and_escapes_text() {
        let scan = sample();
        let html = render_html(&scan, &crate::grade::grade(&scan), None);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.trim_end().ends_with("</html>"));
        assert!(html.contains("Dell Inc. Latitude &lt;7420&gt;"));
        assert!(!html.contains("<7420>"));
        assert!(
            !html.contains("<script"),
            "the page must work without scripts"
        );
    }

    #[test]
    fn page_never_says_healthy_or_not_stolen() {
        let scan = sample();
        let html = render_html(&scan, &crate::grade::grade(&scan), None).to_lowercase();
        assert!(!html.contains("healthy"));
        assert!(!html.contains("not stolen"));
    }

    #[test]
    fn hands_on_results_are_shown() {
        let scan = sample();
        let hands_on = HandsOn {
            version: 1,
            tests: vec![TestResult {
                kind: TestKind::Keyboard,
                outcome: Outcome::Problems,
                tested_at: 0,
                summary: "77 of 78 keys registered".into(),
                problems: vec!["Key K did not register".into()],
                detail: serde_json::Value::Null,
            }],
        };
        let html = render_html(&scan, &crate::grade::grade(&scan), Some(&hands_on));
        assert!(html.contains("Hands-on tests"));
        assert!(html.contains("Key K did not register"));
    }

    #[test]
    fn dates_are_written_out() {
        assert_eq!(date_of(0), "on an unknown date");
        assert_eq!(date_of(86_400 * 365), "1 January 1971");
        assert_eq!(date_of(1_709_164_800), "29 February 2024");
    }
}
