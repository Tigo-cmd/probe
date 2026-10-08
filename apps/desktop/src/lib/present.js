// Presentation rules for scans and grades.
//
// The wording here is part of the product, mirroring crates/hwprobe/src/report.rs:
// every value carries its epistemic class, a missing value says "not reported",
// a clean result states its coverage, and no result is ever declared a clean bill.

/** @typedef {'green' | 'amber' | 'red' | 'not_graded'} Verdict */

export const VERDICTS = {
  red: { label: 'Red', headline: 'Problems found', rank: 3 },
  amber: { label: 'Amber', headline: 'Needs attention', rank: 2 },
  green: { label: 'Green', headline: 'No faults detected', rank: 1 },
  not_graded: { label: 'Not graded', headline: 'Not graded: evidence missing', rank: 0 },
};

export const AXES = {
  function: { label: 'Function', blurb: 'Drives and core hardware' },
  battery: { label: 'Battery', blurb: 'Capacity and wear' },
  encumbrance: { label: 'Encumbrance', blurb: 'Locks, management and ex-corporate markers' },
};

export const PROVENANCE = {
  measured: { label: 'Measured', hint: 'Read from a sensor or controller.' },
  claimed: { label: 'Claimed', hint: 'Read from rewritable firmware; can be altered.' },
  inferred: { label: 'Inferred', hint: 'Derived from other values, with an error band.' },
  not_assessable: { label: 'Not assessable', hint: 'Software cannot determine this.' },
};

/** @param {string} v */
export function verdict(v) {
  return VERDICTS[v] ?? VERDICTS.not_graded;
}

/** @param {string} a */
export function axisLabel(a) {
  return AXES[a]?.label ?? a;
}

/** @param {string} p */
export function provenance(p) {
  return PROVENANCE[p] ?? { label: p, hint: '' };
}

/** "No faults detected in N checks", as the text report says it. */
export function axisSummary(axis) {
  switch (axis.verdict) {
    case 'green':
      return `No faults detected in ${axis.checks} ${axis.checks === 1 ? 'check' : 'checks'}`;
    case 'not_graded':
      return 'Not graded';
    default:
      return `${axis.checks} ${axis.checks === 1 ? 'check' : 'checks'}`;
  }
}

/** Findings worst first; the order within a verdict is kept. */
export function sortFindings(findings) {
  return findings
    .map((f, i) => ({ f, i }))
    .sort((a, b) => verdict(b.f.verdict).rank - verdict(a.f.verdict).rank || a.i - b.i)
    .map(({ f }) => f);
}

/**
 * A field ready to show: either its formatted value, or "not reported" with
 * the reason. A missing value is never shown as a default.
 */
export function field(f, format = (v) => String(v)) {
  if (!f || f.value === null || f.value === undefined) {
    return {
      missing: true,
      text: 'Not reported',
      note: f?.note ?? null,
      provenance: f?.provenance ?? null,
      source: f?.source ?? '',
    };
  }
  return {
    missing: false,
    text: format(f.value),
    note: f.note ?? null,
    provenance: f.provenance,
    source: f.source ?? '',
  };
}

export function formatBytes(bytes) {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(2)} TB`;
  return `${Math.round(bytes / 1e9)} GB`;
}

export function formatGiB(bytes) {
  return `${(bytes / 2 ** 30).toFixed(1)} GiB`;
}

export function formatCapacity(c) {
  return `${c.value.toLocaleString('en-US')} ${c.unit === 'milliwatt_hours' ? 'mWh' : 'mAh'}`;
}

/**
 * Full-charge capacity as a share of design, when both are reported in the
 * same unit. `null` otherwise: never guessed.
 */
export function capacityShare(battery) {
  const d = battery.design_capacity?.value;
  const f = battery.full_charge_capacity?.value;
  if (!d || !f || d.unit !== f.unit || d.value <= 0) return null;
  return Math.round((f.value / d.value) * 100);
}

export const TRANSPORTS = {
  nvme: 'NVMe',
  sata: 'SATA',
  usb: 'USB (external)',
  other: 'Other',
};

/** What the scan says about a drive's health data. */
export function driveHealth(drive) {
  const h = drive.health;
  if (h.kind === 'unavailable') return { read: false, text: h.reason };
  if (h.kind === 'nvme') {
    return {
      read: true,
      text: `NVMe health log read: ${h.percentage_used}% of rated life used, ${h.media_errors} media errors, ${h.power_on_hours.toLocaleString('en-US')} power-on hours`,
    };
  }
  return { read: true, text: `SMART attribute table read: ${h.attributes.length} attributes` };
}

export function scannedAt(unixSeconds) {
  if (!unixSeconds) return 'time not recorded';
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
}

export const OS_NAMES = { linux: 'Linux', windows: 'Windows', macos: 'macOS' };

export function osName(os) {
  return OS_NAMES[os] ?? os;
}

/** What to tell someone whose scan ran without privilege. */
export function elevationHint(os) {
  switch (os) {
    case 'windows':
      return 'Run probe as administrator to read drive health and NVMe serials.';
    case 'macos':
      return 'Run probe with sudo to check the firmware password on Intel Macs.';
    default:
      return 'Run probe with sudo to read serials, drive health and the ACPI tables.';
  }
}

/** A file name for saving a scan, e.g. probe-PF0ABCDE-2026-10-06.json. */
export function defaultFileName(scan, extension) {
  const serial = (scan.identity?.serial?.value ?? 'scan').replace(/[^A-Za-z0-9_-]/g, '');
  const date = scan.started_at ? new Date(scan.started_at * 1000).toISOString().slice(0, 10) : 'undated';
  return `probe-${serial || 'scan'}-${date}.${extension}`;
}

export const COMPONENT_NAMES = {
  smbios_uuid: 'SMBIOS UUID',
  board_serial: 'Board serial',
  system_serial: 'System serial',
  drive_serial: 'Drive serial',
  battery_serial: 'Battery serial',
  mac: 'Network MAC',
};

/** One line per divergence between an earlier scan and this one. */
export function describeDivergence(d) {
  const name = COMPONENT_NAMES[d.component] ?? d.component;
  if (!d.before) return `${name} added: ${d.after}`;
  if (!d.after) return `${name} no longer present: ${d.before}`;
  return `${name} changed: ${d.before} → ${d.after}`;
}

export const ORIGINS = {
  live: 'Scanned here',
  imported: 'Opened from a file',
};

export const SYNC_STATES = {
  local: 'On this computer only',
  pending: 'Queued to share',
  synced: 'Shared',
  failed: 'Sharing failed, will retry',
};

/** "3 scans on this computer · none shared". */
export function historyLine(counts) {
  const total = counts.local + counts.pending + counts.synced + counts.failed;
  if (total === 0) return 'No scans stored yet';
  const scans = `${total} ${total === 1 ? 'scan' : 'scans'} on this computer`;
  const shared = counts.synced + counts.pending + counts.failed;
  if (shared === 0) return `${scans} · none shared`;
  const parts = [];
  if (counts.synced) parts.push(`${counts.synced} shared`);
  if (counts.pending) parts.push(`${counts.pending} queued`);
  if (counts.failed) parts.push(`${counts.failed} failed`);
  return `${scans} · ${parts.join(', ')}`;
}

/** "Same laptop, 14 days earlier". */
export function relativeAge(earlierSeconds, laterSeconds) {
  const days = Math.round((laterSeconds - earlierSeconds) / 86400);
  if (days <= 0) return 'earlier the same day';
  return `${days} ${days === 1 ? 'day' : 'days'} earlier`;
}

/**
 * What to say when a scan did not run. A dismissed administrator prompt is a
 * choice, not an error; everything else says what failed.
 */
export function scanFailure(e) {
  if (e && typeof e === 'object' && e.kind === 'cancelled') {
    return {
      cancelled: true,
      text: 'Administrator access was not given. You can scan without it; drive health, serials and firmware checks will be missing.',
    };
  }
  const message = e && typeof e === 'object' && 'message' in e ? e.message : String(e);
  if (e && typeof e === 'object' && e.kind === 'unavailable') {
    return {
      cancelled: true,
      text: `Administrator access is not available (${message}). You can scan without it; drive health, serials and firmware checks will be missing.`,
    };
  }
  return { cancelled: false, text: `The scan could not run: ${message}` };
}
