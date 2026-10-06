# probe

Used-laptop condition and encumbrance verification. A scan runs on the laptop
being inspected, offline, from a single binary on a USB stick, and produces a
graded report that separates what was **measured** from what the firmware
merely **claims** and from what software **cannot assess**.

It does not determine whether a laptop is stolen. No public register of laptop
serials exists, and the report says so.

## Layout

| Path | What it is |
| --- | --- |
| `crates/hwprobe` | The extraction and grading core. Independent of any GUI. |
| `crates/hwprobe-cli` | `hwprobe`, the technician CLI and CI harness, built on the core. |

Planned, not yet in this repo: the Tauri 2 + Svelte desktop shell, the
per-OS privileged helper (deliberately deferred: v1 needs no driver), the
macOS backend, the local SQLite scan queue, and the sync backend.

## Using the CLI

```sh
cargo build --release
sudo ./target/release/hwprobe            # scan, grade, print the report
sudo ./target/release/hwprobe scan -o scan.json
./target/release/hwprobe grade scan.json # re-grade a saved scan offline
./target/release/hwprobe diff old.json new.json  # fingerprint divergence
```

Root is needed on Linux for serials, the SMBIOS UUID, drive health and the
ACPI tables. On Windows, run from an administrator prompt for drive health and
NVMe serials; identity, battery and encumbrance reads work without it. Either
way an unprivileged scan still runs and reports what it could not read.

Exit status: `0` green, `1` amber, `2` red, `3` not graded.

## What the core does today

- **Data model** (`model.rs`). Every value is a `Field` tagged `measured`,
  `claimed`, `inferred` or `not_assessable`, with the interface it came from.
  Missing values stay missing; they are never defaulted to a healthy number.
- **Linux backend** (`backend/linux.rs`). SMBIOS via sysfs (OEM placeholder
  strings rejected), CPU and RAM from procfs, battery capacity and cycles from
  `power_supply`, EDID panel identity, physical NIC MACs, NVMe health via the
  admin Get Log Page ioctl, SATA SMART via SG_IO ATA pass-through. Drives behind
  USB bridges are labelled and never graded as internal. Reads the ACPI WPBT.
- **Windows backend** (`backend/windows.rs`). No WMI, COM or driver. SMBIOS
  from the raw firmware table, storage descriptors and NVMe health and
  Identify through `IOCTL_STORAGE_QUERY_PROPERTY`, SATA SMART through
  `SMART_RCV_DRIVE_DATA`, the battery class IOCTLs, monitor EDID through
  SetupAPI, burned-in MACs of hardware adapters from IP Helper. NVMe serials
  come from Identify Controller so they match what Linux reports.
- **Encumbrance signals** (`backend/signals.rs`). SMBIOS asset tag and WPBT on
  both OSes; on Windows also MDM enrolment, Microsoft Entra ID join, Active
  Directory join, cached Autopilot profile and the Absolute (Computrace) agent.
  Reports name the organisation, never the previous user. A check that could
  not be read emits a probe note and no signal; it is never a clean result.
- **Parsers** (`parse/`). NVMe health log and Identify, ATA SMART attribute
  table, EDID base block, SMBIOS structure table, ACPI WPBT, and the Windows
  storage, battery and processor query buffers. Pure functions over untrusted
  bytes that reject rather than panic, tested on every OS.
- **Grader** (`grade.rs`, versioned as `GRADER_VERSION`). Three axes (function,
  battery, encumbrance); the headline is the worst axis, never an average.
  Every finding carries the rule's evidence basis. Storage uses the five
  Backblaze/Google SMART attributes and NVMe critical warning, media errors,
  spare and wear. Battery uses state of health against design capacity and
  cycle count, and treats `cycle_count == 0` and full charge exactly equal to
  design as *not reported*. Host-writes-versus-power-on-hours plausibility is
  flagged as an inferred signal. Encumbrance: management enrolment, Entra ID
  join, Autopilot profile and Absolute agent are red; asset tag, WPBT and
  domain join are amber; an unknown signal is red. A clean Linux encumbrance
  result is not graded, because the strong signals are not visible there.
- **Fingerprint** (`fingerprint.rs`). Composite of SMBIOS UUID, board and system
  serial, drive and battery serials, and MACs. Divergence between scans is the
  tamper signal.
- **Report** (`report.rs`). Plain-text rendering that states coverage ("no faults
  detected in N checks"), lists not-assessable checks by name, and carries the
  standing caveats (the ~36% of failed drives with no SMART errors, single
  snapshot, fuel-gauge calibration, no theft determination).

## Rules for contributors

- A missing value is never a pass. If a read fails, record a `ProbeNote`.
- Never write "healthy" or "not stolen" anywhere in output. A test enforces this.
- Changing any grading rule or threshold means bumping `GRADER_VERSION`.
  Grades from different versions are not comparable.
- Parsers take untrusted bytes and must not panic on any input.
- Never add a test that writes to a drive, erases, or saturates writes on
  the machine under inspection.
