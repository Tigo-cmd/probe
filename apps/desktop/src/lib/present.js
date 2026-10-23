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
  function: { label: 'Drives', blurb: 'Storage errors and wear' },
  battery: { label: 'Battery', blurb: 'Capacity and wear' },
  encumbrance: { label: 'Ownership', blurb: 'Locks, management and ex-company markers' },
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

// ------------------------------------------------------------ assessment

/** The kinds of use the Rust assessment rates, in its order. */
export const USE_CASES = {
  everyday: { phrase: 'everyday use', label: 'Everyday', title: 'Everyday use', blurb: 'Browsing, email and streaming', icon: 'globe' },
  work: { phrase: 'work', label: 'Work', title: 'Office and video calls', blurb: 'Documents, spreadsheets and meetings', icon: 'briefcase' },
  study: { phrase: 'school', label: 'School', title: 'School and travel', blurb: 'Notes, classes and a day unplugged', icon: 'backpack' },
  programming: { phrase: 'coding', label: 'Coding', title: 'Programming', blurb: 'Editors, builds and dev tools', icon: 'code' },
  creative: { phrase: 'photo editing', label: 'Photo', title: 'Photo and design', blurb: 'Photo editing, illustration and layout', icon: 'brush' },
  video: { phrase: 'video editing', label: 'Video', title: 'Video editing and 3D', blurb: 'Editing, rendering and 3D work', icon: 'film' },
  gaming: { phrase: 'gaming', label: 'Gaming', title: 'Gaming', blurb: 'Recent PC games', icon: 'gamepad' },
};

export const FITS = {
  great: { label: 'Great fit', short: 'Great', tone: 'green' },
  good: { label: 'Good fit', short: 'Good', tone: 'green' },
  limited: { label: 'Works, with compromises', short: 'Limited', tone: 'amber' },
  poor: { label: 'Not a good match', short: 'Poor', tone: 'red' },
  blocked: { label: 'Do not buy yet', short: 'Blocked', tone: 'red' },
  unknown: { label: 'Not enough information', short: 'Unknown', tone: 'grey' },
};

export const BANDS = {
  excellent: 'Excellent condition',
  good: 'Good condition',
  fair: 'Fair condition',
  poor: 'Poor condition',
};

/** @param {string} u */
export function useCase(u) {
  return USE_CASES[u] ?? { label: u, title: u, blurb: '', icon: 'globe' };
}

/** @param {string} f */
export function fit(f) {
  return FITS[f] ?? FITS.unknown;
}

/** The colour family for a verdict: green, amber, red or grey. */
export function verdictTone(v) {
  return v === 'green' || v === 'amber' || v === 'red' ? v : 'grey';
}

/** The colour family for a 0–100 score, for charts without a verdict. */
export function scoreTone(score) {
  if (score === null || score === undefined) return 'grey';
  if (score >= 75) return 'green';
  if (score >= 45) return 'amber';
  return 'red';
}

/**
 * Stroke geometry for a ring chart of radius `r`. A missing value draws an
 * empty ring: it is never shown as full.
 */
export function ring(value, r) {
  const c = 2 * Math.PI * r;
  const v = typeof value === 'number' && Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 0;
  return { circumference: c, offset: c * (1 - v / 100) };
}

export const TIERS = ['Not rated', 'Basic', 'Entry', 'Mainstream', 'Strong', 'High-end'];

export function tierLabel(t) {
  return TIERS[t ?? 0] ?? TIERS[0];
}

export const STORAGE_KINDS = {
  nvme: 'NVMe SSD',
  sata_ssd: 'SATA SSD',
  hdd: 'Hard drive',
  unknown: 'Internal drive',
};

const VENDORS = [
  [/^lenovo/i, 'Lenovo'],
  [/^dell/i, 'Dell'],
  [/^(hp|hewlett)/i, 'HP'],
  [/^asus/i, 'ASUS'],
  [/^apple/i, 'Apple'],
  [/^acer/i, 'Acer'],
  [/^microsoft/i, 'Microsoft'],
  [/^micro-star|^msi/i, 'MSI'],
  [/^samsung/i, 'Samsung'],
  [/^framework/i, 'Framework'],
  [/^razer/i, 'Razer'],
  [/^toshiba|^dynabook/i, 'Dynabook'],
  [/^fujitsu/i, 'Fujitsu'],
  [/^google/i, 'Google'],
  [/^huawei/i, 'Huawei'],
  [/^xiaomi/i, 'Xiaomi'],
  [/^lg /i, 'LG'],
];

/**
 * The name a buyer knows the machine by. Lenovo keeps the marketing name in
 * the version string ("ThinkPad X1 Carbon 5th") and a part number in the
 * model ("20HRCTO1WW"); prefer whichever reads as a name.
 */
export function machineName(identity) {
  const raw = identity?.vendor?.value ?? '';
  const vendor = VENDORS.find(([re]) => re.test(raw))?.[1] ?? raw.trim();
  const model = identity?.model?.value?.trim() ?? '';
  const version = identity?.version?.value?.trim() ?? '';
  const readable = (s) => /[a-z]/.test(s) && /\s/.test(s);
  const name = readable(version) && !readable(model) ? version : model || version;
  return { vendor, name: name || 'Unidentified laptop' };
}

/** Uses ordered best first; unrated ones last, in their original order. */
export function rankUses(uses) {
  return uses
    .map((u, i) => ({ u, i }))
    .sort((a, b) => (b.u.score ?? -1) - (a.u.score ?? -1) || a.i - b.i)
    .map(({ u }) => u);
}

/** "Best for everyday use and office work", or null when nothing fits well. */
export function bestForLine(bestFor) {
  if (!bestFor?.length) return null;
  const names = bestFor.slice(0, 3).map((u) => useCase(u).phrase ?? u);
  const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names.at(-1)}` : names[0];
  return `Best for ${list}`;
}

/** One sentence under the score saying how complete it is. */
export function coverageLine(condition) {
  const total = condition.areas.length;
  if (condition.score === null || condition.score === undefined) {
    return 'No score: none of the three areas could be read.';
  }
  const parts = [];
  if (condition.scored_areas < total) {
    parts.push(`Based on ${condition.scored_areas} of ${total} areas; the others could not be read.`);
  }
  if (condition.cap !== null && condition.cap !== undefined) {
    parts.push(`Capped at ${condition.cap} because of this.`);
  }
  return parts.join(' ') || 'Based on drives, battery and ownership checks.';
}

/** Battery capacity colours follow the grader's bands: 80% and up, 70–79%, below 70%. */
export function batteryTone(percent) {
  if (percent === null || percent === undefined) return 'grey';
  if (percent > 105) return 'amber';
  if (percent >= 80) return 'green';
  if (percent >= 70) return 'amber';
  return 'red';
}

/** "5,210 hours (about 7 months powered on)". */
export function powerOnText(hours) {
  const h = Number(hours);
  if (!Number.isFinite(h)) return 'Not reported';
  const days = h / 24;
  const span =
    days >= 365 ? `${(days / 365).toFixed(1)} years` : days >= 60 ? `${Math.round(days / 30)} months` : `${Math.round(days)} days`;
  return `${h.toLocaleString('en-US')} hours (about ${span} powered on)`;
}

/** NVMe data units are 1000 × 512 bytes. */
export function nvmeWritten(units) {
  const bytes = Number(units) * 512_000;
  if (!Number.isFinite(bytes)) return 'Not reported';
  return bytes >= 1e12 ? `${(bytes / 1e12).toFixed(1)} TB` : `${Math.round(bytes / 1e9)} GB`;
}

export const SMART_NAMES = {
  5: 'Reallocated sectors',
  187: 'Reported uncorrectable',
  188: 'Command timeouts',
  197: 'Pending sectors',
  198: 'Offline uncorrectable',
};

/** An ATA attribute's raw value as an event count, as the Rust grader reads it. */
export function ataCount(attr) {
  const raw = Number(attr.raw);
  return attr.id === 188 ? raw % 2 ** 16 : raw % 2 ** 32;
}

/** "Shared 3 scans", or why a pass stopped. */
export function syncReport(r) {
  if (!r) return '';
  if (r.stopped) return `Sharing paused: ${r.stopped}`;
  const done = r.sent + r.already;
  if (done === 0 && r.failed === 0) return 'Nothing waiting to be shared.';
  const parts = [];
  if (done) parts.push(`Shared ${done} ${done === 1 ? 'scan' : 'scans'}`);
  if (r.failed) parts.push(`${r.failed} failed and will be retried`);
  return parts.join('; ') + '.';
}

export const MATCH_NAMES = { uuid: 'UUID', serial: 'serial', board_serial: 'board serial' };

/** "Matched on UUID and serial". */
export function matchedOn(keys) {
  const names = (keys ?? []).map((k) => MATCH_NAMES[k] ?? k);
  if (!names.length) return '';
  return `Matched on ${names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names.at(-1)}` : names[0]}`;
}
