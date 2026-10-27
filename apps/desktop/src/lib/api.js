// The bridge to the Rust side. Every scan, grade, comparison and history
// operation is computed there; this module only moves data and asks the user
// for file locations.

import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { defaultFileName } from './present.js';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// Design preview only: `npm run dev` in a plain browser with ?demo shows
// fixtures produced by the Rust code. Production builds drop every branch
// below that reads them, so a shipped app can never display data that was
// not read from a machine.
export const demoMode =
  import.meta.env.DEV && !inTauri && typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('demo');

async function demo(name) {
  const fixtures = {
    opened: () => import('./fixtures/demo.json'),
    history: () => import('./fixtures/demo-history.json'),
    compare: () => import('./fixtures/demo-compare.json'),
  };
  return (await fixtures[name]()).default;
}

const SCAN_FILTER = [{ name: 'probe scan', extensions: ['json'] }];

function requireTauri() {
  if (!inTauri) throw new Error('probe must run as the desktop app to read hardware.');
}

/**
 * Whether a full scan needs the OS's administrator prompt here.
 * @returns {Promise<{elevated: boolean, can_elevate: boolean, method: string, reason: string|null}|null>}
 */
export async function privilegeStatus() {
  if (import.meta.env.DEV && demoMode) {
    return { elevated: false, can_elevate: true, method: 'macOS administrator password', reason: null };
  }
  if (!inTauri) return null;
  return invoke('privilege_status');
}

/**
 * Scan this machine. With `full`, the OS asks for an administrator password
 * first (unless the app already has those rights); a dismissed prompt
 * rejects with `{kind: 'cancelled'}`. The result is kept in the local history.
 * @returns {Promise<{scan: object, grade: object, record: object|null, earlier: object[], history_error: string|null}>}
 */
export async function runScan({ full = false } = {}) {
  if (import.meta.env.DEV && demoMode) {
    await new Promise((r) => setTimeout(r, 1200));
    return demo('opened');
  }
  requireTauri();
  return invoke(full ? 'run_elevated_scan' : 'run_scan');
}

/** Ask for a saved scan, re-grade it and add it to the history. `null` if cancelled. */
export async function openSavedScan() {
  requireTauri();
  const path = await open({ filters: SCAN_FILTER, multiple: false, directory: false });
  if (!path) return null;
  return { path, opened: await invoke('open_scan', { path }) };
}

/** The local history, or `null` outside the app. */
export async function listHistory() {
  if (import.meta.env.DEV && demoMode) return demo('history');
  if (!inTauri) return null;
  return invoke('list_history');
}

export async function openStored(id) {
  if (import.meta.env.DEV && demoMode) {
    const d = await demo('opened');
    if (d.record?.id === id) return d;
    throw new Error('Only the newest sample scan opens in the design preview.');
  }
  requireTauri();
  return invoke('open_stored', { id });
}

export async function deleteStored(id) {
  if (import.meta.env.DEV && demoMode) return;
  requireTauri();
  return invoke('delete_stored', { id });
}

export async function labelStored(id, label) {
  if (import.meta.env.DEV && demoMode) return { ...(await demo('opened')).record, label };
  requireTauri();
  return invoke('label_stored', { id, label: label || null });
}

/** Compare this scan with a stored earlier scan of the same machine. */
export async function compareStored(id, scan) {
  if (import.meta.env.DEV && demoMode) return demo('compare');
  requireTauri();
  return invoke('compare_stored', { id, scan });
}

export async function saveScanAs(scan) {
  requireTauri();
  const path = await save({ defaultPath: defaultFileName(scan, 'json'), filters: SCAN_FILTER });
  if (!path) return null;
  await invoke('save_scan', { path, scan });
  return path;
}

export async function saveTextReportAs(scan, id = null) {
  requireTauri();
  const path = await save({
    defaultPath: defaultFileName(scan, 'html'),
    filters: [
      { name: 'Report (web page)', extensions: ['html'] },
      { name: 'Plain text report', extensions: ['txt'] },
    ],
  });
  if (!path) return null;
  await invoke('save_text_report', { path, scan, id });
  return path;
}

/** Compare this scan's identifiers with a saved scan file. */
export async function compareWithSaved(scan) {
  requireTauri();
  const path = await open({ filters: SCAN_FILTER, multiple: false, directory: false });
  if (!path) return null;
  return { path, comparison: await invoke('compare_with', { path, scan }) };
}

// ------------------------------------------------------------ team sharing
// Uploads and lookups happen in Rust; the token never comes back here.

const DEMO_TEAM = { configured: true, server: 'https://sync.example-repairs.co.uk', device: 'Front desk' };

/** @returns {Promise<{configured: boolean, server: string|null, device: string|null}|null>} */
export async function teamStatus() {
  if (import.meta.env.DEV && demoMode) return DEMO_TEAM;
  if (!inTauri) return null;
  return invoke('team_status');
}

/** Check the server and token, then keep them. Rejects with the reason. */
export async function teamConnect(server, token) {
  if (import.meta.env.DEV && demoMode) return { ...DEMO_TEAM, server };
  requireTauri();
  return invoke('team_connect', { server, token });
}

export async function teamDisconnect() {
  if (import.meta.env.DEV && demoMode) return { configured: false, server: null, device: null };
  requireTauri();
  return invoke('team_disconnect');
}

/** Queue a stored scan for sharing and try to send it now. */
export async function shareStored(id) {
  if (import.meta.env.DEV && demoMode) {
    const d = await demo('opened');
    return { record: { ...d.record, sync: 'synced', synced_at: d.record.stored_at }, report: { sent: 1, already: 0, failed: 0, stopped: null }, error: null };
  }
  requireTauri();
  return invoke('share_stored', { id });
}

export async function withdrawStored(id) {
  if (import.meta.env.DEV && demoMode) return { ...(await demo('opened')).record, sync: 'local' };
  requireTauri();
  return invoke('withdraw_stored', { id });
}

/** Send everything queued. */
export async function syncNow() {
  if (import.meta.env.DEV && demoMode) return { sent: 0, already: 0, failed: 0, stopped: null };
  requireTauri();
  return invoke('sync_now');
}

/** Scans on the team server that claim the same machine. */
export async function teamLookup(scan, exclude) {
  if (import.meta.env.DEV && demoMode) {
    return {
      truncated: false,
      matches: [
        {
          content_id: 'd'.repeat(64),
          started_at: scan.started_at - 86400 * 41,
          received_at: scan.started_at - 86400 * 41,
          device: 'Workshop',
          os: 'windows',
          machine: 'LENOVO 20HRCTO1WW',
          serial: scan.identity?.serial?.value ?? null,
          headline: 'green',
          matched_on: ['uuid', 'serial'],
        },
      ],
    };
  }
  requireTauri();
  return invoke('team_lookup', { scan, exclude: exclude ?? null });
}

// ------------------------------------------------------- hands-on tests

/**
 * Record one finished hands-on test against a stored scan. Returns the scan
 * re-assessed with the result, as Rust computes it.
 */
export async function saveHandsOnResult(id, result, current) {
  if (import.meta.env.DEV && demoMode) {
    // Design preview: keep the result, but the assessment is not recomputed.
    const tests = (current.hands_on?.tests ?? []).filter((t) => t.kind !== result.kind).concat(result);
    return { ...current, hands_on: { version: 1, tests } };
  }
  requireTauri();
  return invoke('save_hands_on_result', { id, result });
}

/** Full screen for the screen test; falls back to the browser API. */
export async function setFullscreen(on) {
  if (inTauri) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().setFullscreen(on);
    return;
  }
  try {
    if (on && !document.fullscreenElement) await document.documentElement.requestFullscreen();
    if (!on && document.fullscreenElement) await document.exitFullscreen();
  } catch {
    // Fullscreen refused: the test still runs inside the window.
  }
}

// --------------------------------------------------------- live readings
// Sampled repeatedly by the longer tests. The design preview simulates a
// laptop so every screen can be seen without hardware.

const demoStart = Date.now();
const demoSeconds = () => (Date.now() - demoStart) / 1000;

export async function livePower() {
  if (import.meta.env.DEV && demoMode) {
    const t = demoSeconds();
    // Unplugged for the first 6 s of each 20 s, then charging; drains ~7 W.
    const onAc = t % 20 > 6;
    const energy = Math.round(41_000 - t * 2);
    return { on_ac: onAc, charging: onAc, percent: (energy / 51_000) * 100, energy_mwh: energy, full_mwh: 51_000, rate_mw: onAc ? 38_000 : -7_200, adapter_watts: onAc ? 65 : null, source: 'demo' };
  }
  requireTauri();
  return invoke('live_power');
}

export async function liveCpu() {
  if (import.meta.env.DEV && demoMode) {
    const t = demoSeconds();
    return { mhz: 2400 + Math.round(Math.sin(t) * 300), max_mhz: 3900, temp_c: 62 + Math.min(20, t % 60), limit_pct: null, source: 'demo' };
  }
  requireTauri();
  return invoke('live_cpu');
}

export async function liveRadios() {
  if (import.meta.env.DEV && demoMode) {
    await new Promise((r) => setTimeout(r, 600));
    return [
      { kind: 'wifi', name: 'Intel Wireless 8265', enabled: true, connected: true },
      { kind: 'bluetooth', name: 'Intel Bluetooth', enabled: true, connected: null },
    ];
  }
  requireTauri();
  return invoke('live_radios');
}

let demoUsbCalls = 0;
export async function liveUsb() {
  if (import.meta.env.DEV && demoMode) {
    // A stick appears on the 3rd poll and leaves on the 6th, repeatedly.
    demoUsbCalls++;
    const base = [{ id: '1-5/04f2:b5c0', name: 'Integrated Camera' }];
    return demoUsbCalls % 6 >= 3 ? [...base, { id: `1-2/0781:5583/${Math.floor(demoUsbCalls / 6)}`, name: 'SanDisk Ultra Fit' }] : base;
  }
  requireTauri();
  return invoke('live_usb');
}

export async function readSpeed() {
  if (import.meta.env.DEV && demoMode) {
    await new Promise((r) => setTimeout(r, 2500));
    return { mb_per_s: 1640.4, bytes: 1 << 30, files: 9, seconds: 0.65, method: 'demo' };
  }
  requireTauri();
  return invoke('read_speed');
}
