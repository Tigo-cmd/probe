import { describe, expect, it } from 'vitest';
import {
  coverage,
  createKeyTracker,
  keyboardLayout,
  keyboardResult,
  physicalResult,
  PHYSICAL_ITEMS,
  pointerResult,
  POINTER_STEPS,
  refreshRate,
  reportRate,
  requiredKeys,
  screenResult,
  speakerResult,
  speakerRounds,
} from './handson.js';

describe('keyboard test', () => {
  const layout = keyboardLayout('windows');
  const all = (t) => {
    let time = 0;
    for (const key of requiredKeys(layout)) {
      t.keydown(key.code, (time += 200), true);
      t.keyup(key.code, (time += 80));
    }
  };

  it('passes when every required key registers once, without needing Fn or F-keys', () => {
    const t = createKeyTracker();
    all(t);
    const r = keyboardResult(layout, t);
    expect(r.outcome).toBe('passed');
    expect(requiredKeys(layout).some((k) => k.code === 'Fn' || k.code === 'F5')).toBe(false);
    expect(requiredKeys(keyboardLayout('macos')).some((k) => k.code === 'MetaLeft')).toBe(true);
  });

  it('calls untried keys incomplete until the tester says they are broken', () => {
    const t = createKeyTracker();
    t.keydown('KeyA', 0, true);
    t.keyup('KeyA', 50);
    expect(keyboardResult(layout, t).outcome).toBe('incomplete');
    const r = keyboardResult(layout, t, { broken: true });
    expect(r.outcome).toBe('problems');
    expect(r.problems).toContain('Key K did not register');
  });

  it('flags a key that bounces, and ignores auto-repeat while held', () => {
    const t = createKeyTracker();
    all(t);
    t.keydown('KeyE', 10_000, true);
    t.keydown('KeyE', 10_030, true); // auto-repeat while held: ignored
    t.keyup('KeyE', 10_060);
    for (const at of [10_080, 10_100]) {
      t.keydown('KeyE', at, true);
      t.keyup('KeyE', at + 10);
    }
    const r = keyboardResult(layout, t);
    expect(r.outcome).toBe('problems');
    expect(r.problems).toEqual(['Key E sometimes registers twice']);
  });

  it('notes keys outside the layout without failing on them', () => {
    const t = createKeyTracker();
    all(t);
    t.keydown('PageDown', 99_000, false);
    expect(keyboardResult(layout, t).detail.other_keys).toEqual(['PageDown']);
  });
});

describe('pointer test', () => {
  const done = Object.fromEntries(POINTER_STEPS.map((s) => [s.id, 'done']));

  it('passes when every step completed, and names the report rate', () => {
    const r = pointerResult(done, { rate_hz: 125 });
    expect(r.outcome).toBe('passed');
    expect(r.summary).toContain('125');
  });

  it('reports a pointer that moved by itself as a problem', () => {
    const r = pointerResult({ ...done, still: 'failed' });
    expect(r.outcome).toBe('problems');
    expect(r.problems[0]).toMatch(/on its own/);
    expect(pointerResult({ ...done, scroll: 'skipped' }).outcome).toBe('incomplete');
  });

  it('measures coverage and the report rate from event timing', () => {
    expect(coverage(new Set([0, 1, 2]), 2, 3)).toBeCloseTo(0.5);
    const times = Array.from({ length: 60 }, (_, i) => i * 8);
    expect(reportRate(times)).toBe(125);
    expect(reportRate([1, 2])).toBeNull();
  });
});

describe('screen test', () => {
  it('snaps the measured refresh rate to a real panel rate', () => {
    const at = (gap) => Array.from({ length: 60 }, (_, i) => i * gap);
    expect(refreshRate(at(16.7))).toBe(60);
    expect(refreshRate(at(8.4))).toBe(120);
    expect(refreshRate(at(6.94))).toBe(144);
    expect(refreshRate([0, 16])).toBeNull();
  });

  it('turns chosen issues and marked spots into problems', () => {
    expect(screenResult([], [], { refresh_hz: 60 }).summary).toMatch(/60 Hz/);
    const r = screenResult(['bleed'], [{ x: 0.1, y: 0.2, colour: 'Black' }]);
    expect(r.outcome).toBe('problems');
    expect(r.problems).toEqual(['Light leaking at the edges', '1 spot marked on the screen']);
  });
});

describe('speaker test', () => {
  it('plays left and right in a random order, then both', () => {
    expect(speakerRounds(() => 0.1)).toEqual(['left', 'right', 'both']);
    expect(speakerRounds(() => 0.9)).toEqual(['right', 'left', 'both']);
  });

  it('catches a silent or swapped side, and only passes a full run', () => {
    expect(speakerResult({ left: 'left', right: 'right', both: 'both' }, false).outcome).toBe('passed');
    const silent = speakerResult({ left: 'left', right: 'nothing', both: 'left' }, false);
    expect(silent.problems[0]).toBe('No sound from the right speaker');
    expect(speakerResult({ left: 'right', right: 'left', both: 'both' }, false).problems).toHaveLength(2);
    expect(speakerResult({ left: 'left' }, null).outcome).toBe('incomplete');
  });
});

describe('physical inspection', () => {
  it('lists problems with their notes and needs every item for a pass', () => {
    const fine = Object.fromEntries(PHYSICAL_ITEMS.map((i) => [i.id, 'fine']));
    expect(physicalResult(fine).outcome).toBe('passed');
    const r = physicalResult({ ...fine, hinges: 'problem' }, { hinges: ' left hinge loose ' });
    expect(r.problems).toEqual(['Hinges: left hinge loose']);
    expect(physicalResult({ case: 'fine' }).outcome).toBe('incomplete');
  });
});
