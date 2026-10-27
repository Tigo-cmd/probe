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

import {
  batteryResult,
  brightness,
  cameraResult,
  chargerResult,
  drainEstimate,
  hoursText,
  newDevices,
  performanceResult,
  portsResult,
  storageResult,
  TESTS,
  wirelessResult,
} from './handson.js';

describe('the test list', () => {
  it('has fourteen tests in two groups, each kind once', () => {
    expect(TESTS).toHaveLength(14);
    expect(new Set(TESTS.map((t) => t.kind)).size).toBe(14);
    expect(TESTS.every((t) => t.group === 'quick' || t.group === 'long')).toBe(true);
  });
});

describe('webcam and microphone', () => {
  const cam = { frames: true, width: 1280, height: 720, dark: false, clear: true };
  it('passes only when both work and the picture was confirmed', () => {
    expect(cameraResult(cam, { heard: true }).outcome).toBe('passed');
    expect(cameraResult({ ...cam, clear: null }, { heard: true }).outcome).toBe('incomplete');
    expect(cameraResult({ ...cam, dark: true }, { heard: true }).problems[0]).toMatch(/black/);
    expect(cameraResult(cam, { heard: false }).problems).toEqual(['The microphone did not react to sound']);
    expect(cameraResult({ error: 'NotAllowedError' }, { heard: true }).outcome).toBe('problems');
  });
  it('measures brightness from pixels', () => {
    expect(brightness(new Uint8ClampedArray(64 * 4).fill(0))).toBe(0);
    expect(Math.round(brightness(new Uint8ClampedArray(64 * 4).fill(255)))).toBe(255);
  });
});

describe('wireless and ports', () => {
  const wifi = { kind: 'wifi', name: 'wlan0', enabled: true, connected: true };
  const bt = { kind: 'bluetooth', name: 'hci0', enabled: true, connected: null };
  it('needs both radios on, and asks about nearby networks', () => {
    expect(wirelessResult([wifi, bt], true).outcome).toBe('passed');
    expect(wirelessResult([wifi], true).problems).toEqual(['No Bluetooth found']);
    expect(wirelessResult([wifi, { ...bt, enabled: false }], true).summary).toBe('Bluetooth switched off');
    expect(wirelessResult([wifi, bt], false).outcome).toBe('problems');
  });
  it('finds a newly plugged device and scores the ports', () => {
    const before = [{ id: 'a', name: 'Webcam' }];
    expect(newDevices(before, [...before, { id: 'b', name: 'Stick' }])).toEqual([{ id: 'b', name: 'Stick' }]);
    expect(portsResult(['ok', 'ok']).outcome).toBe('passed');
    expect(portsResult(['ok', 'failed']).problems).toEqual(['Port 2 did not detect a device']);
    expect(portsResult(['ok', 'skipped']).outcome).toBe('incomplete');
  });
});

describe('performance, storage and charger', () => {
  it('catches a laptop that slows down as it heats up', () => {
    const steady = Array(40).fill(100);
    expect(performanceResult(steady).outcome).toBe('passed');
    const fading = [...Array(20).fill(100), ...Array(20).fill(60)];
    expect(performanceResult(fading).problems[0]).toMatch(/Slowed down by 40%/);
    expect(performanceResult(steady, [70, 99]).problems).toEqual(['Processor reached 99 °C']);
    expect(performanceResult(steady, [], [100, 60]).outcome).toBe('problems');
    expect(performanceResult(Array(5).fill(100)).outcome).toBe('incomplete');
  });
  it('judges read speed against the kind of drive', () => {
    expect(storageResult({ mb_per_s: 1800 }, 'nvme').outcome).toBe('passed');
    expect(storageResult({ mb_per_s: 120 }, 'nvme').outcome).toBe('problems');
    expect(storageResult({ mb_per_s: 90 }, 'hdd').summary).toBe('Reads at 90 MB/s, normal for a hard drive');
    expect(storageResult({ mb_per_s: 10 }, 'unknown').outcome).toBe('passed');
  });
  it('accepts a full battery that does not charge, and flags a dead charger', () => {
    const base = { unplugged: true, plugged: true, charging: true, rate_w: 40, adapter_w: 65, percent: 50 };
    expect(chargerResult(base).summary).toBe('Charger works (65 W charger, charging at 40 W)');
    expect(chargerResult({ ...base, plugged: false }).outcome).toBe('problems');
    expect(chargerResult({ ...base, charging: false }).outcome).toBe('problems');
    expect(chargerResult({ ...base, charging: false, percent: 100 }).outcome).toBe('passed');
    expect(chargerResult({ ...base, rate_w: 3 }).outcome).toBe('problems');
  });
});

describe('battery drain', () => {
  const samples = (minutes, dropMwhPerMin, pctPerMin = 0.2) =>
    Array.from({ length: minutes * 6 + 1 }, (_, i) => ({
      t: i * 10,
      energy_mwh: 50_000 - (dropMwhPerMin * i) / 6,
      percent: 80 - (pctPerMin * i) / 6,
      rate_mw: null,
    }));
  it('estimates running time from energy used', () => {
    // 100 mWh a minute is 6 W; 60 Wh full lasts 10 hours.
    const e = drainEstimate(samples(10, 100), 60_000);
    expect(e.watts).toBeCloseTo(6);
    expect(e.runtime_h).toBeCloseTo(10);
    expect(batteryResult(samples(10, 100), 60_000).summary).toBe('About 10 h 0 min on a full charge at light use drawing about 6.0 W');
  });
  it('flags short running time, sudden drops, and runs that are too short', () => {
    expect(batteryResult(samples(10, 1000), 20_000).outcome).toBe('problems');
    const jumpy = samples(10, 100);
    jumpy[20].percent -= 8;
    expect(batteryResult(jumpy, 60_000).problems[0]).toMatch(/jumps/);
    expect(batteryResult(samples(2, 100), 60_000).outcome).toBe('incomplete');
    expect(hoursText(1.5)).toBe('1 h 30 min');
  });
});

import { graphicsResult, memoryResult, memoryTarget } from './handson.js';

describe('memory test', () => {
  it('tests about half the memory, between 256 MB and 2 GB', () => {
    expect(memoryTarget(16 * 1024 ** 3)).toBe(2048);
    expect(memoryTarget(4 * 1024 ** 3)).toBe(1792);
    expect(memoryTarget(null)).toBe(1792);
    expect(memoryTarget(256 * 1024 ** 2)).toBe(256);
  });
  it('fails on any value read back wrong', () => {
    const r = memoryResult({ testedMb: 2048, totalMb: 16000, errors: 3, examples: [], finished: true });
    expect(r.outcome).toBe('problems');
    expect(r.problems[0]).toContain('3 values');
  });
  it('passes a clean, finished run and says how much was checked', () => {
    const r = memoryResult({ testedMb: 2048, totalMb: 16384, errors: 0, examples: [], finished: true });
    expect(r.outcome).toBe('passed');
    expect(r.summary).toContain('2.0 GB of 16.0 GB');
  });
  it('is incomplete when stopped early', () => {
    expect(memoryResult({ testedMb: 2048, totalMb: 16384, errors: 0, finished: false }).outcome).toBe('incomplete');
  });
});

describe('graphics test', () => {
  const steady = Array(60).fill(58);
  it('passes a steady run', () => {
    expect(graphicsResult({ perSecond: steady, glitches: 0, lost: false }).outcome).toBe('passed');
  });
  it('catches overheating, glitches and driver crashes', () => {
    const slowing = [...Array(30).fill(60), ...Array(30).fill(30)];
    expect(graphicsResult({ perSecond: slowing, glitches: 0, lost: false }).summary).toContain('slowed by 50%');
    expect(graphicsResult({ perSecond: steady, glitches: 2, lost: false }).outcome).toBe('problems');
    expect(graphicsResult({ perSecond: steady.slice(0, 10), glitches: 0, lost: true }).summary).toContain('crashed');
  });
  it('says so when there is no graphics acceleration', () => {
    expect(graphicsResult({ unavailable: true, perSecond: [] }).outcome).toBe('incomplete');
  });
});
