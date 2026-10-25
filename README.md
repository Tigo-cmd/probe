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
| `crates/scanstore` | Local scan history and sync queue (SQLite, compiled in). |
| `crates/elevate` | Runs one command with administrator rights through the OS's own prompt. |
| `crates/probe-sync` | `probe-sync`, the self-hosted team server that shared scans sync to. |
| `crates/syncclient` | The client the app uses to upload queued scans and query the team server. |
| `apps/desktop` | The desktop app: Tauri 2 shell (`src-tauri`) and Svelte 5 frontend (`src`). |


## Testing on a real laptop

Every push to the development branch builds unsigned test installers (the
"Test builds" workflow). Open the run in GitHub Actions and download the
artifact for your system from its Artifacts section:

| Artifact | Contains |
| --- | --- |
| `probe-windows` | `.msi` and `-setup.exe` installers, plus `hwprobe.exe` |
| `probe-macos` | a `.dmg` for Apple silicon and Intel Macs, plus the CLI |
| `probe-linux` | a `.deb` package, plus the CLI |

They are not signed. Windows SmartScreen needs "More info, Run anyway"; on a
Mac, open the app once with right-click, Open, or run
`xattr -cr /Applications/probe.app` if macOS calls it damaged.

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
NVMe serials; identity, battery and encumbrance reads work without it. On
macOS, root is needed only for the firmware password check on Intel Macs. In
every case an unprivileged scan still runs and reports what it could not read.

Exit status: `0` green, `1` amber, `2` red, `3` not graded.

## The desktop app

```sh
cd apps/desktop
npm install
npm run tauri dev           # run against this machine
npm run tauri build         # installers for the current OS
npm test                    # presentation rules and wording guard
```

On Linux, building needs the WebKitGTK development packages
(`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`). The app is its own Cargo
workspace so the core crates never depend on a webview.

All scanning, grading and comparison happen in Rust; the frontend only
presents what it receives. A saved scan is re-graded on open, so its verdicts
always come from the grader the app ships with. The report shows the headline
and the three axes, every finding with its provenance tag and basis, the
machine's identifiers, drives, batteries, displays and adapters, what software
cannot assess, the standing caveats and the probe notes. It saves the raw scan
as JSON (readable by the CLI), saves the text report, and compares this scan's
identifiers with an earlier one to flag swapped parts.

Every scan the app runs or opens is kept in a local history
(`scans.sqlite3` in the app's data folder; the home screen shows the path).
The report lists earlier scans that claim to be the same machine (same SMBIOS
UUID, system serial or board serial) and compares with one click. Each scan
takes an optional note, such as the listing or seller.

`npm run dev` with `?demo` in a plain browser shows sample data for design
work. Production builds drop that path, so a shipped app can only display data
read from a machine. `src/lib/fixtures/demo.json` is produced by the Rust
grader; a test fails when it drifts.

### Condition score and advice for your use

The report opens on a **condition score** out of 100, drawn as a ring, with a
smaller ring for each area: drives, battery and ownership. Below it the buyer
picks what the laptop is for (everyday, work, school, coding, photo, video,
gaming) and sees a fit rating with its reasons, plus a bar chart ranking every
use. Spec tiles, battery and drive charts follow, and every check the grader
ran is still listed in full further down. The design follows Apple's Human
Interface Guidelines and switches to dark mode with the system.

Both come from `crates/hwprobe/src/assess.rs`, not the frontend, and follow
the same rules as the grader:

- **The score never contradicts the grade.** It is a weighted mean of the
  areas (drives 45, battery 25, ownership 30), then capped by the headline:
  red caps it at 39, amber at 79.
- **Missing evidence is left out, not counted as full marks.** An area that
  could not be read is named and excluded; the report says "based on 2 of 3
  areas". With no evidence there is no score.
- **Condition limits every use.** A management lock makes every use
  "Do not buy yet"; a failing drive caps every use at 25.
- **Estimates are labelled.** Processor and graphics performance tiers come
  from model names, which are claims, so those reasons are tagged inferred.
  The thresholds are this app's own conventions, versioned as
  `ASSESS_VERSION`.

Scans now also record graphics adapters (Linux via sysfs and the system's
`pci.ids`, Windows via SetupAPI, macOS via `system_profiler`). Scans saved
before this have none and still load; the graphics part of the advice then
says it was not recorded.

## Full scans without running the app as administrator

Serials, drive health and firmware tables need administrator rights. The app
never takes them itself. "Scan this laptop" asks the OS to start a second copy
of the app's own binary with administrator rights and one argument,
`--elevated-scan <file>`: UAC on Windows, the administrator password dialog
on macOS, polkit (`pkexec`) on Linux. That copy scans, writes the JSON and
exits before any window or web view exists; the app reads the file and
carries on unprivileged.

- The hand-off file sits in a fresh owner-only directory the app creates and
  removes. The elevated copy accepts no other argument, refuses any path not
  shaped like a hand-off file, and creates the file without ever overwriting.
- A dismissed or refused prompt is a choice, not an error: the app says what
  will be missing and offers a scan without administrator rights. A partial
  report offers "Rescan with administrator rights".
- If the app already runs as administrator or root, it scans directly.
- An AppImage cannot be re-run as root, and some Linux systems lack polkit;
  the app says so and points to `sudo hwprobe` instead.

The mechanism is `crates/elevate`; its quoting (AppleScript, Windows command
lines) and outcome rules are tested on every OS. The Linux path was run end to
end here: an unprivileged user, a real `pkexec`, granted and refused.

### Hands-on tests

From any report, **Hands-on tests** opens five guided checks. Results are
kept beside the scan in the local history (never inside it, so the scan's
content id does not change) and appear in the report and the text report.
A test that finds a problem holds the condition score to 79, limits every
use it matters to, and leads the next steps.

| Test | How it works |
| --- | --- |
| Keyboard | A laptop-shaped map (Mac or PC) lights up each key as it registers, matched by physical position so any keyboard language works. Flags keys that register twice (worn switches) and keys held down for seconds (stuck). F-keys are optional; Fn never reaches software. |
| Touchpad and mouse | Seven steps that advance by themselves when they see the right event: sweep the whole pad, click, right-click, double-click, drag, scroll both ways, then hands off for three seconds to catch a pointer that drifts on its own. Measures the report rate and notices a touchscreen. |
| Screen | Full-screen black, white, red, green, blue, grey and a gradient. The tester can click any bad spot to mark it. Refresh rate is measured from frame timing; resolution and scaling are read. |
| Speakers | A chime from the left and right in a random order; the tester says where they heard it, so a silent or swapped side is caught. Then a low-to-high sweep for buzzing. |
| Physical inspection | Case, hinges, battery swelling, ports and charging, signs of liquid, screws and seals, webcam and microphone, smell and heat. |

The scoring and wording for these live in `apps/desktop/src/lib/handson.js`
(tested) and `crates/hwprobe/src/handson.rs`.

### Appearance

The switch at the top right picks Match system, Light or Dark. The choice is
remembered on that computer and also sets the window frame.

## Team sharing (probe-sync)

A shop or team can run one server so its computers share scans and can ask
"has anyone here scanned this laptop before?" Nothing is shared by
connecting: each scan is shared from its report, by choice.

Run the server on any machine (one binary, SQLite compiled in):

```sh
cargo build --release -p probe-sync
./target/release/probe-sync token add "Front desk"   # prints a token once
./target/release/probe-sync serve --db probe-sync.sqlite3 --listen 127.0.0.1:8640
```

It speaks plain HTTP on localhost. To reach it from other computers, put it
behind a TLS reverse proxy (for example Caddy: `reverse_proxy 127.0.0.1:8640`);
the app refuses plain HTTP except to localhost, because scans contain serial
numbers. In the app, open Team sharing on the home screen and enter the server
address and the token. `probe-sync token list` shows devices and
`probe-sync token revoke NAME` cuts one off.

The API (`/v1`, every call but `health` needs `Authorization: Bearer TOKEN`):

| Call | What it does |
| --- | --- |
| `GET /v1/health` | Server and grader versions, and which device the token belongs to |
| `PUT /v1/scans/{content_id}` | Upload a scan. The id must be the SHA-256 of the body, so retries are no-ops and altered uploads are refused. 201 new, 200 already stored |
| `GET /v1/scans/{content_id}` | The scan exactly as uploaded |
| `DELETE /v1/scans/{content_id}` | Remove a scan; only the device that uploaded it can |
| `GET /v1/lookup?uuid=&serial=&board_serial=&exclude=` | Scans that claim the same machine, newest first, each graded now by the server's grader and saying which identifiers matched |

Rules: tokens are stored only as hashes; verdicts are computed when read, never
stored; a database from a newer server version is refused. The app uploads
queued scans when you share one, when you press Sync now, and at start-up; a
revoked token or unreachable server stops the pass and leaves scans queued.
The token is kept in `sync.json` in the app's data folder (owner-only on
macOS and Linux).

## Scan history and sync queue

`crates/scanstore` keeps scans in SQLite with the C library compiled in, so
there is still nothing to install. The rules:

- **Nothing is shared unasked.** A scan starts `local`. Only an explicit
  request moves it to `pending`; the sync client takes
  pending scans oldest first and marks them `synced` or `failed` (failed ones
  are retried). `withdraw` takes a scan back out before it is sent.
- **Scans are immutable and content-addressed.** Each is stored exactly as
  captured, keyed by the SHA-256 of its JSON. Storing it twice is a no-op, so
  a sync retry can never create a duplicate.
- **Verdicts are a cache.** Opening the store with a different grader version
  re-grades every row, so the history never shows an outdated verdict.
- **Newer files are refused, not rewritten.** A history written by a newer
  schema stops with an error instead of being migrated backwards.
- **History failure never blocks a scan.** If the file cannot be opened, the
  app still scans and reports, and says why the scan was not kept.

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
- **macOS backend** (`backend/macos.rs`). Uses only tools that ship with
  macOS: `ioreg -a`, `system_profiler -xml`, `profiles`, `firmwarepasswd` and
  `sysctl`. Identity from the platform expert, battery from AppleSmartBattery
  (on Apple silicon `MaxCapacity` is a percentage and is never graded as a
  capacity), drives, burned-in MACs and EDID. macOS exposes only a pass/fail
  SMART summary, so Mac drives are listed but not graded. All interpretation
  is pure and tested on every OS; only a small `sys` module runs commands.
- **Encumbrance signals** (`backend/signals.rs`). SMBIOS asset tag and WPBT on
  both OSes; on Windows also MDM enrolment, Microsoft Entra ID join, Active
  Directory join, cached Autopilot profile and the Absolute (Computrace) agent;
  on macOS Activation Lock, Automated Device Enrollment, MDM enrolment and the
  Intel firmware password.
  Reports name the organisation, never the previous user. A check that could
  not be read emits a probe note and no signal; it is never a clean result.
- **Parsers** (`parse/`). NVMe health log and Identify, ATA SMART attribute
  table, XML property lists, EDID base block, SMBIOS structure table, ACPI WPBT, and the Windows
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
  join, Autopilot profile, Absolute agent, Activation Lock, Automated Device
  Enrollment and firmware password are red; asset tag, WPBT and
  domain join are amber; an unknown signal is red. A clean Linux encumbrance
  result is not graded, because the strong signals are not visible there; nor
  is a clean Mac whose Activation Lock state could not be read.
- **Fingerprint** (`fingerprint.rs`). Composite of SMBIOS UUID, board and system
  serial, drive and battery serials, and MACs. Divergence between scans is the
  tamper signal.
- **Assessment** (`assess.rs`, versioned as `ASSESS_VERSION`). Condition score
  capped by the grade, and fit for seven kinds of use from processor and
  graphics tiers, memory, storage, screen and battery. See above.
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
