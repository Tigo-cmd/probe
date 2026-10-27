//! Can this computer run Windows 11? Checked against Microsoft's published
//! minimum requirements: a supported processor, 4 GB of memory, 64 GB of
//! storage, UEFI with Secure Boot capability, and TPM 2.0.
//!
//! Two common failures are settings, not hardware: a TPM built into the
//! processor (Intel PTT, AMD fTPM) that is turned off, and Secure Boot left
//! off. Those make the result "fixable" rather than "not supported".

use serde::{Deserialize, Serialize};

use crate::model::{Provenance, Scan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// Meets every requirement that could be read.
    Ready,
    /// Meets the hardware requirements once a firmware setting is changed.
    Fixable,
    NotSupported,
    /// A requirement could not be read, so no verdict.
    Unknown,
    /// A Mac: Windows 11 is not supported on Mac hardware.
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub label: String,
    /// `None` when it could not be read.
    pub met: Option<bool>,
    /// What was found, in plain words.
    pub detail: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Windows11 {
    pub readiness: Readiness,
    /// One sentence for the headline.
    pub summary: String,
    pub requirements: Vec<Requirement>,
    /// What to change in the firmware settings, when the result is fixable.
    pub fixes: Vec<String>,
}

pub fn check(scan: &Scan) -> Windows11 {
    if scan.os == "macos" {
        return Windows11 {
            readiness: Readiness::NotApplicable,
            summary: "Windows 11 is not supported on Mac hardware".into(),
            requirements: Vec::new(),
            fixes: Vec::new(),
        };
    }
    let mut reqs = Vec::new();
    let mut fixes: Vec<String> = Vec::new();

    // Processor.
    let brand = scan.cpu.brand.get().map(String::as_str);
    let cpu_ok = brand.and_then(cpu_supported);
    reqs.push(Requirement {
        label: "Processor".into(),
        met: cpu_ok,
        detail: match (brand, cpu_ok) {
            (None, _) => "not reported".into(),
            (Some(b), Some(true)) => format!("{} is on the supported list", tidy(b)),
            (Some(b), Some(false)) => format!("{} is older than Windows 11 supports", tidy(b)),
            (Some(b), None) => {
                format!("{}: support could not be worked out from its name", tidy(b))
            }
        },
        provenance: Provenance::Inferred,
    });

    // Memory: usable memory reads a little under what is installed.
    let gib = scan
        .memory
        .total_bytes
        .get()
        .map(|b| *b as f64 / (1u64 << 30) as f64);
    reqs.push(Requirement {
        label: "Memory (4 GB)".into(),
        met: gib.map(|g| g >= 3.5),
        detail: gib
            .map(|g| format!("{g:.1} GB usable"))
            .unwrap_or_else(|| "not reported".into()),
        provenance: Provenance::Measured,
    });

    // Storage: the largest internal drive.
    let gb = scan
        .storage
        .iter()
        .filter(|d| !d.removable)
        .filter_map(|d| d.capacity_bytes.get().copied())
        .max()
        .map(|b| b as f64 / 1e9);
    reqs.push(Requirement {
        label: "Storage (64 GB)".into(),
        met: gb.map(|g| g >= 64.0),
        detail: gb
            .map(|g| format!("{g:.0} GB drive"))
            .unwrap_or_else(|| "not reported".into()),
        provenance: Provenance::Claimed,
    });

    // Firmware: UEFI, then Secure Boot.
    let p = &scan.platform;
    let uefi = p.uefi.get().copied();
    let secure_boot = p.secure_boot.get().copied();
    reqs.push(Requirement {
        label: "UEFI firmware".into(),
        met: uefi,
        detail: match uefi {
            Some(true) => "started in UEFI mode".into(),
            Some(false) => "started in legacy BIOS mode".into(),
            None => "not read".into(),
        },
        provenance: Provenance::Measured,
    });
    if uefi == Some(false) {
        fixes.push(
            "Switch the firmware from legacy (CSM) to UEFI mode. The system drive must be converted to GPT first, or Windows will not start.".into(),
        );
    }
    let sb_met = match (uefi, secure_boot) {
        (_, Some(true)) => Some(true),
        // UEFI firmware can turn Secure Boot on: capability is the requirement.
        (Some(true), Some(false)) => Some(true),
        (Some(false), _) => Some(false),
        _ => None,
    };
    reqs.push(Requirement {
        label: "Secure Boot".into(),
        met: sb_met,
        detail: match (uefi, secure_boot) {
            (_, Some(true)) => "on".into(),
            (Some(true), Some(false)) => "supported, currently off".into(),
            (Some(false), _) => "needs UEFI mode".into(),
            _ => "not read".into(),
        },
        provenance: Provenance::Measured,
    });
    if uefi == Some(true) && secure_boot == Some(false) {
        fixes.push("Turn on Secure Boot in the firmware settings.".into());
    }

    // TPM.
    let tpm = p.tpm_version.get().map(String::as_str);
    reqs.push(Requirement {
        label: "TPM 2.0".into(),
        met: tpm.map(|v| v == "2.0"),
        detail: match tpm {
            Some("2.0") => "TPM 2.0 present".into(),
            Some("none") => "no TPM found".into(),
            Some(v) => format!("TPM {v}"),
            None => "not read".into(),
        },
        provenance: Provenance::Measured,
    });
    // Supported processors all have a TPM 2.0 built in, often turned off.
    let tpm_fixable = tpm == Some("none") && cpu_ok == Some(true);
    if tpm_fixable {
        fixes.push(
            "Turn on the processor's built-in TPM in the firmware settings (Intel PTT or AMD fTPM).".into(),
        );
    }

    let failed: Vec<&Requirement> = reqs.iter().filter(|r| r.met == Some(false)).collect();
    let unknown = reqs.iter().any(|r| r.met.is_none());
    let only_settings = failed.iter().all(|r| {
        r.label == "UEFI firmware"
            || r.label == "Secure Boot"
            || (r.label == "TPM 2.0" && tpm_fixable)
    });
    let (readiness, summary) = if !failed.is_empty() && !only_settings {
        let names: Vec<&str> = failed.iter().map(|r| short(&r.label)).collect();
        (
            Readiness::NotSupported,
            format!(
                "Does not meet Windows 11's requirements: {}",
                names.join(", ")
            ),
        )
    } else if !failed.is_empty() {
        (
            Readiness::Fixable,
            "Can run Windows 11 after a change in the firmware settings".into(),
        )
    } else if unknown {
        (
            Readiness::Unknown,
            "Some Windows 11 requirements could not be read".into(),
        )
    } else if secure_boot == Some(false) {
        (
            Readiness::Ready,
            "Meets Windows 11's requirements; Secure Boot is off but can be turned on".into(),
        )
    } else {
        (Readiness::Ready, "Meets Windows 11's requirements".into())
    };
    // Fixes only matter when they lead to a computer that can run it.
    match readiness {
        Readiness::Fixable => {}
        Readiness::Ready => fixes.retain(|f| f.starts_with("Turn on Secure Boot")),
        _ => fixes.clear(),
    }
    Windows11 {
        readiness,
        summary,
        requirements: reqs,
        fixes,
    }
}

fn short(label: &str) -> &str {
    label.split(" (").next().unwrap_or(label)
}

fn tidy(brand: &str) -> String {
    brand
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace(" CPU", "")
        .split(" @ ")
        .next()
        .unwrap_or(brand)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether Microsoft's processor list includes this chip, from its name.
/// `None` when the name is not enough to tell.
pub fn cpu_supported(brand: &str) -> Option<bool> {
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
    let digits = |s: &str| -> String { s.chars().take_while(|c| c.is_ascii_digit()).collect() };

    if b.contains("snapdragon") {
        return Some(true);
    }
    if b.contains("apple") {
        return Some(false);
    }
    if b.contains("intel") {
        // Core Ultra and the 2023 "Core 3/5/7" names are all recent.
        if words.contains(&"ultra") {
            return Some(true);
        }
        if let Some(i) = words
            .iter()
            .position(|w| ["i3", "i5", "i7", "i9"].contains(w))
        {
            let model = digits(words.get(i + 1).copied().unwrap_or(""));
            let gen: Option<u32> = match model.len() {
                3 => Some(1),
                4 if model.starts_with('1') => model[..2].parse().ok(),
                4 => model[..1].parse().ok(),
                5 => model[..2].parse().ok(),
                _ => None,
            };
            return gen.map(|g| g >= 8);
        }
        if after("core").is_some_and(|m| ["3", "5", "7", "9"].contains(&m)) {
            return Some(true);
        }
        // Low-cost N and J chips: the 2017-on ones (N4000, J4105, N5100,
        // N100) are listed; the N3xxx and J3xxx generation is not.
        if let Some(w) = words.iter().find(|w| {
            w.len() >= 3
                && (w.starts_with('n') || w.starts_with('j'))
                && w[1..].chars().all(|c| c.is_ascii_digit())
        }) {
            let num = &w[1..];
            return Some(num.len() <= 3 || !num.starts_with(['1', '2', '3']));
        }
        // Older Celeron, Pentium and Atom chips mostly predate the list;
        // the recent ones (Gold 6405U, Silver N5030) cannot be told apart
        // reliably from their names.
        return None;
    }
    if b.contains("amd") {
        if b.contains("ryzen") {
            if b.contains("ryzen ai") {
                return Some(true);
            }
            let model = words
                .iter()
                .find(|w| w.len() >= 4 && w.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .copied()?;
            let num = digits(model);
            let suffix = &model[num.len()..];
            return match num.chars().next()? {
                '1' => Some(false),
                // Ryzen 2000 laptop and G chips are first-generation Zen.
                '2' => Some(
                    !(suffix.starts_with('u')
                        || suffix.starts_with('h')
                        || suffix.starts_with('g')),
                ),
                _ => Some(true),
            };
        }
        if b.contains("epyc") || b.contains("threadripper") {
            return None;
        }
        // A-series, E-series, FX and older Athlons predate the list.
        if words.iter().any(|w| {
            w.starts_with("fx")
                || w.starts_with("a4")
                || w.starts_with("a6")
                || w.starts_with("a8")
                || w.starts_with("a9")
                || w.starts_with("a10")
                || w.starts_with("a12")
                || w.starts_with("e1")
                || w.starts_with("e2")
        }) {
            return Some(false);
        }
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn processors_by_name() {
        for (brand, want) in [
            ("Intel(R) Core(TM) i7-7600U CPU @ 2.80GHz", Some(false)),
            ("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz", Some(true)),
            ("11th Gen Intel(R) Core(TM) i5-1135G7 @ 2.40GHz", Some(true)),
            ("12th Gen Intel(R) Core(TM) i7-12700H", Some(true)),
            ("Intel(R) Core(TM) i5-4300U CPU @ 1.90GHz", Some(false)),
            ("Intel(R) Core(TM) Ultra 7 155H", Some(true)),
            ("Intel(R) N100", Some(true)),
            ("Intel(R) Celeron(R) CPU N3060 @ 1.60GHz", Some(false)),
            ("Intel(R) Celeron(R) N4020 CPU @ 1.10GHz", Some(true)),
            ("Intel(R) Core(TM) 5 120U", Some(true)),
            ("Intel(R) Pentium(R) CPU 4415U @ 2.30GHz", None),
            ("AMD Ryzen 5 2500U with Radeon Vega Mobile Gfx", Some(false)),
            ("AMD Ryzen 7 2700X Eight-Core Processor", Some(true)),
            ("AMD Ryzen 5 1600 Six-Core Processor", Some(false)),
            ("AMD Ryzen 7 5800U with Radeon Graphics", Some(true)),
            ("AMD A9-9425 RADEON R5, 5 COMPUTE CORES 2C+3G", Some(false)),
            (
                "Snapdragon(R) X Elite - X1E78100 - Qualcomm(R) Oryon(TM) CPU",
                Some(true),
            ),
        ] {
            assert_eq!(cpu_supported(brand), want, "{brand}");
        }
    }

    fn scan(cpu: &str, uefi: bool, secure_boot: bool, tpm: &str) -> Scan {
        Scan {
            os: "windows".into(),
            cpu: Cpu {
                brand: Field::claimed(Some(cpu.into()), "test"),
                ..Cpu::default()
            },
            memory: Memory {
                total_bytes: Field::measured(Some(8 << 30), "test"),
            },
            storage: vec![Drive {
                name: "disk0".into(),
                transport: Transport::Nvme,
                rotational: Field::default(),
                removable: false,
                model: Field::default(),
                serial: Field::default(),
                firmware: Field::default(),
                capacity_bytes: Field::claimed(Some(256_000_000_000), "test"),
                health: DriveHealth::Unavailable {
                    reason: "test".into(),
                },
            }],
            platform: Platform {
                uefi: Field::measured(Some(uefi), "test"),
                secure_boot: Field::measured(Some(secure_boot), "test"),
                tpm_version: Field::measured(Some(tpm.into()), "test"),
            },
            ..Scan::default()
        }
    }

    #[test]
    fn ready_fixable_and_not_supported() {
        let ready = check(&scan("Intel(R) Core(TM) i5-8250U", true, true, "2.0"));
        assert_eq!(ready.readiness, Readiness::Ready);

        let tpm_off = check(&scan("Intel(R) Core(TM) i5-8250U", true, false, "none"));
        assert_eq!(tpm_off.readiness, Readiness::Fixable);
        assert!(tpm_off.fixes.iter().any(|f| f.contains("PTT")));
        assert!(tpm_off.fixes.iter().any(|f| f.contains("Secure Boot")));

        let old = check(&scan("Intel(R) Core(TM) i7-7600U", true, true, "2.0"));
        assert_eq!(old.readiness, Readiness::NotSupported);
        assert!(old.summary.contains("Processor"));
        assert!(
            old.fixes.is_empty(),
            "no fix makes an old processor supported"
        );

        let mut mac = scan("Apple M1", true, true, "2.0");
        mac.os = "macos".into();
        assert_eq!(check(&mac).readiness, Readiness::NotApplicable);
    }

    #[test]
    fn unread_firmware_gives_no_verdict() {
        let mut s = scan("Intel(R) Core(TM) i5-8250U", true, true, "2.0");
        s.platform = Platform::default();
        assert_eq!(check(&s).readiness, Readiness::Unknown);
    }
}
