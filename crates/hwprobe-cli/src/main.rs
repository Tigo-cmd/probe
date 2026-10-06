//! Technician command-line tool. Single static binary, no installer.

use std::fs;
use std::process::ExitCode;

use hwprobe::{grade, report, Fingerprint, Scan, Verdict};

const USAGE: &str = "\
usage:
  hwprobe [report]            scan this machine, grade it, print the report
  hwprobe scan [-o FILE]      scan this machine and write the raw scan as JSON
  hwprobe grade FILE [--json] grade a saved scan offline
  hwprobe diff OLD NEW        compare the device fingerprints of two scans

exit status: 0 green, 1 amber, 2 red, 3 not graded, 64 usage error";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] | ["report"] => {
            let scan = hwprobe::scan();
            let g = grade::grade(&scan);
            print!("{}", report::render(&scan, &g));
            Ok(exit_for(g.headline))
        }
        ["scan"] => print_json(&hwprobe::scan()),
        ["scan", "-o", path] => write_json(path, &hwprobe::scan()),
        ["grade", path] => load(path).map(|scan| {
            let g = grade::grade(&scan);
            print!("{}", report::render(&scan, &g));
            exit_for(g.headline)
        }),
        ["grade", path, "--json"] => load(path).and_then(|scan| {
            let g = grade::grade(&scan);
            print_json(&g).map(|_| exit_for(g.headline))
        }),
        ["diff", old, new] => diff(old, new),
        ["-h" | "--help" | "help"] => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        ["-V" | "--version"] => {
            println!(
                "hwprobe {} (grader {})",
                hwprobe::VERSION,
                grade::GRADER_VERSION
            );
            Ok(ExitCode::SUCCESS)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(64))
        }
    };
    result.unwrap_or_else(|e| {
        eprintln!("hwprobe: {e}");
        ExitCode::FAILURE
    })
}

fn exit_for(v: Verdict) -> ExitCode {
    ExitCode::from(match v {
        Verdict::Green => 0,
        Verdict::Amber => 1,
        Verdict::Red => 2,
        Verdict::NotGraded => 3,
    })
}

fn diff(old: &str, new: &str) -> Result<ExitCode, String> {
    let (a, b) = (Fingerprint::of(&load(old)?), Fingerprint::of(&load(new)?));
    let changes = a.diverges_from(&b);
    println!("identifiers matching: {}", a.matching(&b));
    if changes.is_empty() {
        println!("no divergence between identifiers present in both scans");
        return Ok(ExitCode::SUCCESS);
    }
    for c in &changes {
        match (c.before.is_empty(), c.after.is_empty()) {
            (true, _) => println!("{}: added {}", c.component, c.after),
            (_, true) => println!("{}: removed {}", c.component, c.before),
            _ => println!("{}: {} -> {}", c.component, c.before, c.after),
        }
    }
    Ok(ExitCode::from(2))
}

fn load(path: &str) -> Result<Scan, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<ExitCode, String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(ExitCode::SUCCESS)
}

fn write_json<T: serde::Serialize>(path: &str, value: &T) -> Result<ExitCode, String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("{path}: {e}"))?;
    Ok(ExitCode::SUCCESS)
}
