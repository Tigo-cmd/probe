// Hands-on tests: the pure parts (layouts, scoring, result wording), kept
// out of the components so they are tested without a screen.
//
// A result is { kind, outcome, tested_at, summary, problems, detail } with
// outcome 'passed' | 'problems' | 'incomplete', matching hwprobe::handson.

export const TESTS = [
  { kind: 'keyboard', title: 'Keyboard', icon: 'keyboard', minutes: 2, blurb: 'Press every key once. Each one lights up when the laptop registers it.' },
  { kind: 'pointer', title: 'Touchpad and mouse', icon: 'pointer', minutes: 1, blurb: 'Sweep, click, drag and scroll. Each step moves on by itself when it works.' },
  { kind: 'screen', title: 'Screen', icon: 'display', minutes: 1, blurb: 'Full-screen colours to spot dead pixels, bright patches and lines.' },
  { kind: 'speakers', title: 'Speakers', icon: 'speaker', minutes: 1, blurb: 'Sounds play from one side at a time; you say where you heard them.' },
  { kind: 'physical', title: 'Physical inspection', icon: 'checklist', minutes: 2, blurb: 'A guided look at the case, hinges, ports and battery.' },
];

export const OUTCOMES = {
  passed: { label: 'Passed', tone: 'green' },
  problems: { label: 'Problems found', tone: 'red' },
  incomplete: { label: 'Not finished', tone: 'amber' },
};

export function result(kind, outcome, summary, problems = [], detail = {}) {
  return { kind, outcome, tested_at: Math.floor(Date.now() / 1000), summary, problems, detail };
}

// ----------------------------------------------------------------- keyboard

/** A key: physical `code` (layout-independent), label, width in key units. */
const k = (code, label, w = 1, extra = {}) => ({ code, label, w, ...extra });

const LETTERS = (codes) => codes.split('').map((c) => k(`Key${c}`, c));

/**
 * A laptop keyboard by physical position. `optional` keys are often taken
 * by the system (brightness, volume) or need Fn, so they are not required.
 * Fn never reaches software and is shown for orientation only.
 */
export function keyboardLayout(os) {
  const mac = os === 'macos';
  const fRow = [
    k('Escape', 'esc', 1.4),
    ...Array.from({ length: 12 }, (_, i) => k(`F${i + 1}`, `F${i + 1}`, 1, { optional: true })),
  ];
  if (!mac) fRow.push(k('Delete', 'Del', 1.2, { optional: true }));
  const bottom = mac
    ? [
        k('Fn', 'fn', 1, { undetectable: true }),
        k('ControlLeft', 'control', 1.1),
        k('AltLeft', 'option', 1.1),
        k('MetaLeft', 'command', 1.35),
        k('Space', '', 5.2),
        k('MetaRight', 'command', 1.35),
        k('AltRight', 'option', 1.1),
      ]
    : [
        k('ControlLeft', 'Ctrl', 1.3),
        k('Fn', 'Fn', 1, { undetectable: true }),
        k('MetaLeft', 'Win', 1.1),
        k('AltLeft', 'Alt', 1.1),
        k('Space', '', 5),
        k('AltRight', 'Alt', 1.1),
        k('ControlRight', 'Ctrl', 1.1, { optional: true }),
      ];
  return [
    fRow,
    [
      k('Backquote', '`'),
      ...'1234567890'.split('').map((d) => k(`Digit${d}`, d)),
      k('Minus', '-'),
      k('Equal', '='),
      k('Backspace', mac ? 'delete' : 'Backspace', 1.8),
    ],
    [k('Tab', 'tab', 1.5), ...LETTERS('QWERTYUIOP'), k('BracketLeft', '['), k('BracketRight', ']'), k('Backslash', '\\', 1.3)],
    [k('CapsLock', 'caps', 1.8), ...LETTERS('ASDFGHJKL'), k('Semicolon', ';'), k('Quote', "'"), k('Enter', mac ? 'return' : 'Enter', 2)],
    [k('ShiftLeft', 'shift', 2.3), ...LETTERS('ZXCVBNM'), k('Comma', ','), k('Period', '.'), k('Slash', '/'), k('ShiftRight', 'shift', 2.5)],
    [...bottom, k('ArrowLeft', '←'), k('ArrowUp', '↑', 1, { half: true }), k('ArrowDown', '↓', 1, { half: true }), k('ArrowRight', '→')],
  ];
}

/** Keys the test requires: everything detectable and not optional. */
export function requiredKeys(layout) {
  return layout.flat().filter((key) => !key.optional && !key.undetectable);
}

/**
 * Track presses. A key pressed again within `bounceMs` of being released
 * without the person meaning to is "chatter": a worn switch registering
 * twice. Two such bounces flag the key.
 */
export function createKeyTracker({ bounceMs = 35 } = {}) {
  const pressed = new Set();
  const down = new Map();
  const lastUp = new Map();
  const bounces = new Map();
  const extra = new Set();
  return {
    keydown(code, time, known) {
      if (down.has(code)) return;
      down.set(code, time);
      const up = lastUp.get(code);
      if (up !== undefined && time - up < bounceMs) bounces.set(code, (bounces.get(code) ?? 0) + 1);
      pressed.add(code);
      if (!known) extra.add(code);
    },
    keyup(code, time) {
      down.delete(code);
      lastUp.set(code, time);
    },
    /** Keys held down right now and for how long (ms). */
    held(now) {
      return [...down.entries()].map(([code, t]) => ({ code, ms: now - t }));
    },
    pressed,
    extra,
    chattering() {
      return [...bounces.entries()].filter(([, n]) => n >= 2).map(([code]) => code);
    },
  };
}

/** The keyboard result. `broken` is what the tester confirmed as not working. */
export function keyboardResult(layout, tracker, { broken = null } = {}) {
  const label = (code) => {
    const key = layout.flat().find((x) => x.code === code);
    return key?.label?.trim() ? key.label : code;
  };
  const required = requiredKeys(layout);
  const missing = required.filter((key) => !tracker.pressed.has(key.code));
  const chatter = tracker.chattering();
  const problems = [];
  if (broken) for (const key of missing) problems.push(`Key ${label(key.code)} did not register`);
  for (const code of chatter) problems.push(`Key ${label(code)} sometimes registers twice`);
  const detail = {
    required: required.length,
    registered: required.length - missing.length,
    missing: missing.map((key) => key.code),
    chattering: chatter,
    other_keys: [...tracker.extra],
  };
  if (problems.length) {
    const n = missing.length;
    const summary = broken && n ? `${n} ${n === 1 ? 'key' : 'keys'} did not register` : `${chatter.length} ${chatter.length === 1 ? 'key registers' : 'keys register'} twice at times`;
    return result('keyboard', 'problems', summary, problems, detail);
  }
  if (missing.length) {
    return result('keyboard', 'incomplete', `${missing.length} of ${required.length} keys not tried`, [], detail);
  }
  return result('keyboard', 'passed', `All ${required.length} keys registered once each`, [], detail);
}

// ------------------------------------------------------------------ pointer

export const POINTER_STEPS = [
  { id: 'sweep', title: 'Sweep the whole pad', hint: 'Move the pointer over every square until they are all filled. Use the full width of the touchpad.' },
  { id: 'click', title: 'Click', hint: 'Click the circle.' },
  { id: 'right', title: 'Right-click', hint: 'Right-click the circle (two-finger tap or bottom-right corner on most touchpads).' },
  { id: 'double', title: 'Double-click', hint: 'Double-click the circle.' },
  { id: 'drag', title: 'Click and drag', hint: 'Drag the dot into the box and let go.' },
  { id: 'scroll', title: 'Scroll both ways', hint: 'Scroll up and down, then left and right (two fingers on a touchpad).' },
  { id: 'still', title: 'Hands off', hint: 'Take your hands off the laptop for three seconds. The pointer should not move on its own.' },
];

/** Grid coverage as a fraction, from a set of visited cell indexes. */
export function coverage(visited, cols, rows) {
  return visited.size / (cols * rows);
}

/**
 * Pointer report rate from event timestamps (ms) while moving: the median
 * gap, so pauses do not count.
 */
export function reportRate(times) {
  if (times.length < 10) return null;
  const gaps = [];
  for (let i = 1; i < times.length; i++) {
    const g = times[i] - times[i - 1];
    if (g > 0 && g < 100) gaps.push(g);
  }
  if (gaps.length < 5) return null;
  gaps.sort((a, b) => a - b);
  return Math.round(1000 / gaps[Math.floor(gaps.length / 2)]);
}

/** `steps` maps step id to 'done' | 'failed' | 'skipped'. */
export function pointerResult(steps, detail = {}) {
  const failed = POINTER_STEPS.filter((s) => steps[s.id] === 'failed');
  const skipped = POINTER_STEPS.filter((s) => steps[s.id] !== 'done' && steps[s.id] !== 'failed');
  const problems = failed.map((s) => (s.id === 'still' ? 'The pointer moved on its own with nobody touching it' : `${s.title} did not work`));
  if (problems.length) {
    return result('pointer', 'problems', `${problems.length} of ${POINTER_STEPS.length} checks failed`, problems, { ...detail, steps });
  }
  if (skipped.length) {
    return result('pointer', 'incomplete', `${skipped.length} ${skipped.length === 1 ? 'check' : 'checks'} skipped`, [], { ...detail, steps });
  }
  const rate = detail.rate_hz ? `, reporting about ${detail.rate_hz} times a second` : '';
  return result('pointer', 'passed', `Pointer, clicks, drag and scrolling all worked${rate}`, [], { ...detail, steps });
}

// ------------------------------------------------------------------- screen

export const SCREEN_COLOURS = [
  { name: 'Black', css: '#000', look: 'Look for bright dots (stuck pixels) and light leaking in from the edges.' },
  { name: 'White', css: '#fff', look: 'Look for dark dots (dead pixels), yellow patches or pressure marks.' },
  { name: 'Red', css: '#f00', look: 'Look for dots that are not red.' },
  { name: 'Green', css: '#0f0', look: 'Look for dots that are not green.' },
  { name: 'Blue', css: '#00f', look: 'Look for dots that are not blue.' },
  { name: 'Grey', css: '#808080', look: 'Look for darker corners, blotches or a tint across the screen.' },
  { name: 'Gradient', css: 'linear-gradient(90deg, #000, #fff)', look: 'The fade should be smooth, with no lines or bands.' },
];

export const SCREEN_ISSUES = [
  { id: 'dead_pixels', label: 'Dead or stuck pixels' },
  { id: 'bleed', label: 'Light leaking at the edges' },
  { id: 'patches', label: 'Patches, blotches or pressure marks' },
  { id: 'lines', label: 'Lines or flickering' },
  { id: 'uneven', label: 'Uneven colour or tint' },
  { id: 'banding', label: 'Bands in the gradient' },
];

/** Median frame interval → refresh rate, snapped to common panel rates. */
export function refreshRate(frameTimes) {
  if (frameTimes.length < 20) return null;
  const gaps = [];
  for (let i = 1; i < frameTimes.length; i++) gaps.push(frameTimes[i] - frameTimes[i - 1]);
  gaps.sort((a, b) => a - b);
  const hz = 1000 / gaps[Math.floor(gaps.length / 2)];
  const common = [30, 48, 50, 60, 75, 90, 100, 120, 144, 165, 240];
  const near = common.reduce((best, c) => (Math.abs(c - hz) < Math.abs(best - hz) ? c : best));
  return Math.abs(near - hz) <= near * 0.06 ? near : Math.round(hz);
}

export function screenResult(issues, marks, detail = {}) {
  const problems = SCREEN_ISSUES.filter((i) => issues.includes(i.id)).map((i) => i.label);
  if (marks.length && !issues.includes('dead_pixels')) problems.push(`${marks.length} spot${marks.length === 1 ? '' : 's'} marked on the screen`);
  const d = { ...detail, issues, marks };
  if (problems.length) return result('screen', 'problems', problems.join('; '), problems, d);
  const hz = detail.refresh_hz ? ` at ${detail.refresh_hz} Hz` : '';
  return result('screen', 'passed', `No defects seen in ${SCREEN_COLOURS.length} test patterns${hz}`, [], d);
}

// ----------------------------------------------------------------- speakers

/** The play order: left and right in a random order, then both. */
export function speakerRounds(random = Math.random) {
  const sides = random() < 0.5 ? ['left', 'right'] : ['right', 'left'];
  return [...sides, 'both'];
}

/**
 * `answers` maps each played side to what the tester heard:
 * 'left' | 'right' | 'both' | 'nothing'. `rattle` is whether the sweep buzzed.
 */
export function speakerResult(answers, rattle) {
  const problems = [];
  for (const side of ['left', 'right']) {
    const heard = answers[side];
    if (heard === undefined) continue;
    if (heard === 'nothing') problems.push(`No sound from the ${side} speaker`);
    else if (heard !== side) problems.push(`The ${side} channel was heard as ${heard}: speakers may be swapped or one is silent`);
  }
  if (answers.both === 'nothing') problems.push('No sound at all');
  if (rattle) problems.push('Buzzing or rattling during the sweep');
  const detail = { answers, rattle };
  if (problems.length) return result('speakers', 'problems', problems[0], problems, detail);
  if (Object.keys(answers).length < 3 || rattle === null) return result('speakers', 'incomplete', 'Not every sound was checked', [], detail);
  return result('speakers', 'passed', 'Left and right speakers both play, with no rattle', [], detail);
}

// ----------------------------------------------------------------- physical

export const PHYSICAL_ITEMS = [
  { id: 'case', title: 'Case and corners', how: 'Cracks, dents, bent corners or gaps between panels.' },
  { id: 'hinges', title: 'Hinges', how: 'Open and close the lid. It should hold its angle without wobbling, creaking or flexing the screen.' },
  { id: 'battery_bulge', title: 'Battery swelling', how: 'On a flat table, does the laptop rock? Is the touchpad hard to click, or does the bottom bulge? Any of these can mean a swollen battery: do not buy.' },
  { id: 'ports', title: 'Ports and charging', how: 'Plug in the charger and a USB device. They should seat firmly and charging should start immediately.' },
  { id: 'liquid', title: 'Signs of liquid', how: 'Sticky keys, residue around the keyboard, or green or white corrosion near the ports.' },
  { id: 'screws', title: 'Screws and seals', how: 'Missing or stripped screws, or broken warranty seals, mean it has been opened.' },
  { id: 'webcam', title: 'Webcam and microphone', how: 'Open the camera app and record a short clip; check picture and sound.' },
  { id: 'smell', title: 'Smell and heat', how: 'A burnt smell, or the case getting hot while idle, needs explaining.' },
];

/** `answers` maps item id to 'fine' | 'problem' | 'skip'; `notes` to text. */
export function physicalResult(answers, notes = {}) {
  const problems = PHYSICAL_ITEMS.filter((i) => answers[i.id] === 'problem').map((i) =>
    notes[i.id]?.trim() ? `${i.title}: ${notes[i.id].trim()}` : i.title,
  );
  const checked = PHYSICAL_ITEMS.filter((i) => answers[i.id] === 'fine' || answers[i.id] === 'problem').length;
  const detail = { answers, notes };
  if (problems.length) return result('physical', 'problems', `${problems.length} ${problems.length === 1 ? 'problem' : 'problems'} found by inspection`, problems, detail);
  if (checked < PHYSICAL_ITEMS.length) return result('physical', 'incomplete', `${checked} of ${PHYSICAL_ITEMS.length} items checked`, [], detail);
  return result('physical', 'passed', `All ${PHYSICAL_ITEMS.length} items looked fine`, [], detail);
}
