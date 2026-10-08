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
  import.meta.env.DEV && !inTauri && new URLSearchParams(window.location.search).has('demo');

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
 * Scan this machine. The result is kept in the local history.
 * @returns {Promise<{scan: object, grade: object, record: object|null, earlier: object[], history_error: string|null}>}
 */
export async function runScan() {
  if (import.meta.env.DEV && demoMode) {
    await new Promise((r) => setTimeout(r, 1200));
    return demo('opened');
  }
  requireTauri();
  return invoke('run_scan');
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

export async function saveTextReportAs(scan) {
  requireTauri();
  const path = await save({
    defaultPath: defaultFileName(scan, 'txt'),
    filters: [{ name: 'Text report', extensions: ['txt'] }],
  });
  if (!path) return null;
  await invoke('save_text_report', { path, scan });
  return path;
}

/** Compare this scan's identifiers with a saved scan file. */
export async function compareWithSaved(scan) {
  requireTauri();
  const path = await open({ filters: SCAN_FILTER, multiple: false, directory: false });
  if (!path) return null;
  return { path, comparison: await invoke('compare_with', { path, scan }) };
}
