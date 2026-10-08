import { describe, expect, it } from 'vitest';
import demo from './fixtures/demo.json';
import {
  axisSummary,
  capacityShare,
  defaultFileName,
  describeDivergence,
  driveHealth,
  field,
  formatBytes,
  sortFindings,
  verdict,
} from './present.js';

describe('presentation rules', () => {
  it('states coverage for a clean axis and never invents a pass', () => {
    expect(axisSummary({ verdict: 'green', checks: 4 })).toBe('No faults detected in 4 checks');
    expect(axisSummary({ verdict: 'green', checks: 1 })).toBe('No faults detected in 1 check');
    expect(axisSummary({ verdict: 'not_graded', checks: 0 })).toBe('Not graded');
    expect(verdict('something_new').label).toBe('Not graded');
  });

  it('shows a missing value as not reported, with its reason', () => {
    const f = field({ value: null, provenance: 'claimed', source: 'smbios', note: 'OEM placeholder' });
    expect(f).toMatchObject({ missing: true, text: 'Not reported', note: 'OEM placeholder' });
    expect(field(undefined).missing).toBe(true);
    expect(field({ value: 0, provenance: 'measured', source: 's' })).toMatchObject({ missing: false, text: '0' });
  });

  it('puts the worst findings first and keeps order within a verdict', () => {
    const sorted = sortFindings([
      { verdict: 'green', message: 'a' },
      { verdict: 'red', message: 'b' },
      { verdict: 'not_graded', message: 'c' },
      { verdict: 'amber', message: 'd' },
      { verdict: 'red', message: 'e' },
    ]);
    expect(sorted.map((f) => f.message)).toEqual(['b', 'e', 'd', 'a', 'c']);
  });

  it('only computes a capacity share from matching units', () => {
    const cap = (value, unit) => ({ value: { value, unit } });
    expect(capacityShare({ design_capacity: cap(57000, 'milliwatt_hours'), full_charge_capacity: cap(42180, 'milliwatt_hours') })).toBe(74);
    expect(capacityShare({ design_capacity: cap(57000, 'milliwatt_hours'), full_charge_capacity: cap(4000, 'milliamp_hours') })).toBeNull();
    expect(capacityShare({ design_capacity: { value: null }, full_charge_capacity: cap(1, 'milliamp_hours') })).toBeNull();
  });

  it('describes drives and divergences plainly', () => {
    expect(formatBytes(1_000_204_886_016)).toBe('1.00 TB');
    expect(formatBytes(512_110_190_592)).toBe('512 GB');
    expect(driveHealth({ health: { kind: 'unavailable', reason: 'requires root' } })).toEqual({ read: false, text: 'requires root' });
    expect(describeDivergence({ component: 'drive_serial', before: 'A', after: 'B' })).toBe('Drive serial changed: A → B');
    expect(describeDivergence({ component: 'mac', before: '', after: 'aa' })).toBe('Network MAC added: aa');
  });

  it('names saved files after the serial and date, safely', () => {
    expect(defaultFileName({ identity: { serial: { value: 'PF0A/BC DE' } }, started_at: 1791302400 }, 'json')).toBe('probe-PF0ABCDE-2026-10-06.json');
    expect(defaultFileName({ identity: {}, started_at: 0 }, 'txt')).toBe('probe-scan-undated.txt');
  });

  it('reads the demo fixture produced by the Rust grader', () => {
    expect(demo.grade.axes.map((a) => a.axis)).toEqual(['function', 'battery', 'encumbrance']);
    expect(demo.scan.storage[0].health.kind).toBe('nvme');
  });
});

import history from './fixtures/demo-history.json';
import { historyLine, relativeAge } from './present.js';

describe('history', () => {
  it('summarises the queue and says plainly when nothing was shared', () => {
    expect(historyLine({ local: 0, pending: 0, synced: 0, failed: 0 })).toBe('No scans stored yet');
    expect(historyLine({ local: 3, pending: 0, synced: 0, failed: 0 })).toBe('3 scans on this computer · none shared');
    expect(historyLine({ local: 1, pending: 1, synced: 2, failed: 1 })).toBe('5 scans on this computer · 2 shared, 1 queued, 1 failed');
  });

  it('describes how much earlier a scan was', () => {
    expect(relativeAge(0, 14 * 86400)).toBe('14 days earlier');
    expect(relativeAge(0, 86400)).toBe('1 day earlier');
    expect(relativeAge(100, 100)).toBe('earlier the same day');
  });

  it('reads the history fixture produced by the Rust store', () => {
    expect(history.scans).toHaveLength(3);
    expect(history.scans.every((s) => s.sync === 'local')).toBe(true);
    expect(demo.earlier).toHaveLength(1);
  });
});
