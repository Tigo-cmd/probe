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

import { scanFailure } from './present.js';

describe('scan failures', () => {
  it('treats a dismissed administrator prompt as a choice', () => {
    const r = scanFailure({ kind: 'cancelled', message: 'Administrator access was not given.' });
    expect(r.cancelled).toBe(true);
    expect(r.text).toMatch(/scan without it/);
  });

  it('says what failed otherwise', () => {
    const refused = scanFailure({ kind: 'unavailable', message: 'Not authorized' });
    expect(refused.cancelled).toBe(true);
    expect(refused.text).toBe(
      'Administrator access is not available (Not authorized). You can scan without it; drive health, serials and firmware checks will be missing.',
    );
    expect(scanFailure({ kind: 'failed', message: 'disk full' }).text).toBe('The scan could not run: disk full');
    expect(scanFailure(new Error('boom')).text).toBe('The scan could not run: boom');
    expect(scanFailure('plain').text).toBe('The scan could not run: plain');
  });
});

import {
  batteryTone,
  bestForLine,
  coverageLine,
  fit,
  machineName,
  powerOnText,
  rankUses,
  ring,
  tierLabel,
  useCase,
} from './present.js';

describe('assessment presentation', () => {
  it('reads the assessment the Rust code put in the demo fixture', () => {
    const a = demo.assessment;
    expect(a.uses.map((u) => u.use_case)).toEqual(['everyday', 'work', 'study', 'programming', 'creative', 'video', 'gaming']);
    expect(a.uses.every((u) => useCase(u.use_case).title && fit(u.fit).label)).toBe(true);
    expect(a.condition.areas.map((x) => x.axis)).toEqual(['function', 'battery', 'encumbrance']);
  });

  it('draws a missing value as an empty ring, never a full one', () => {
    const empty = ring(null, 10);
    expect(empty.offset).toBeCloseTo(empty.circumference);
    expect(ring(100, 10).offset).toBeCloseTo(0);
    expect(ring(250, 10).offset).toBeCloseTo(0);
    expect(ring(50, 10).offset).toBeCloseTo(empty.circumference / 2);
  });

  it('names the machine the way a buyer knows it', () => {
    const id = (vendor, model, version) => ({ vendor: { value: vendor }, model: { value: model }, version: { value: version } });
    expect(machineName(id('LENOVO', '20HRCTO1WW', 'ThinkPad X1 Carbon 5th'))).toEqual({ vendor: 'Lenovo', name: 'ThinkPad X1 Carbon 5th' });
    expect(machineName(id('Dell Inc.', 'Latitude 7490', null))).toEqual({ vendor: 'Dell', name: 'Latitude 7490' });
    expect(machineName({}).name).toBe('Unidentified computer');
  });

  it('ranks uses best first and keeps unrated ones last', () => {
    const r = rankUses([
      { use_case: 'a', score: null },
      { use_case: 'b', score: 40 },
      { use_case: 'c', score: 90 },
    ]);
    expect(r.map((u) => u.use_case)).toEqual(['c', 'b', 'a']);
  });

  it('says how complete a score is', () => {
    expect(coverageLine({ score: null, areas: [1, 2, 3], scored_areas: 0, cap: null })).toMatch(/No score/);
    expect(coverageLine({ score: 70, areas: [1, 2, 3], scored_areas: 2, cap: null })).toMatch(/2 of 3/);
    expect(coverageLine({ score: 39, areas: [1, 2, 3], scored_areas: 3, cap: 39 })).toMatch(/Capped at 39/);
    expect(bestForLine([])).toBeNull();
    expect(bestForLine(['everyday', 'work', 'study'])).toBe('Best for everyday use, work and school');
  });

  it('uses the grader bands and plain units', () => {
    expect([batteryTone(null), batteryTone(85), batteryTone(74), batteryTone(60), batteryTone(110)]).toEqual(['grey', 'green', 'amber', 'red', 'amber']);
    expect(tierLabel(null)).toBe('Not rated');
    expect(tierLabel(5)).toBe('High-end');
    expect(powerOnText(5210)).toBe('5,210 hours (about 7 months powered on)');
  });
});

import { matchedOn, syncReport } from './present.js';

describe('team sharing', () => {
  it('reports a sync pass plainly, and says when it paused', () => {
    expect(syncReport({ sent: 0, already: 0, failed: 0, stopped: null })).toBe('Nothing waiting to be shared.');
    expect(syncReport({ sent: 2, already: 1, failed: 0, stopped: null })).toBe('Shared 3 scans.');
    expect(syncReport({ sent: 1, already: 0, failed: 1, stopped: null })).toBe('Shared 1 scan; 1 failed and will be retried.');
    expect(syncReport({ sent: 0, already: 0, failed: 1, stopped: 'token revoked' })).toBe('Sharing paused: token revoked');
  });

  it('names the identifiers a team match rests on', () => {
    expect(matchedOn(['uuid', 'serial'])).toBe('Matched on UUID and serial');
    expect(matchedOn(['board_serial'])).toBe('Matched on board serial');
    expect(matchedOn([])).toBe('');
  });
});
