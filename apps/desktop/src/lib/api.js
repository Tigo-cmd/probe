// The bridge to the Rust side. Every scan, grade and comparison is computed
// there; this module only moves data and asks the user for file locations.

import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { defaultFileName } from './present.js';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// Design preview only: `npm run dev` in a plain browser with ?demo shows a
// fixture. Production builds drop this branch, so a shipped app can never
// display data that was not read from a machine.
export const demoMode =
  import.meta.env.DEV && !inTauri && new URLSearchParams(window.location.search).has('demo');

const SCAN_FILTER = [{ name: 'probe scan', extensions: ['json'] }];

function requireTauri() {
  if (!inTauri) throw new Error('probe must run as the desktop app to read hardware.');
}

/** @returns {Promise<{scan: object, grade: object}>} */
export async function runScan() {
  if (import.meta.env.DEV && demoMode) {
    await new Promise((r) => setTimeout(r, 1200));
    return (await import('./fixtures/demo.json')).default;
  }
  requireTauri();
  return invoke('run_scan');
}

/** Ask for a saved scan and re-grade it. `null` if the user cancels. */
export async function openSavedScan() {
  requireTauri();
  const path = await open({ filters: SCAN_FILTER, multiple: false, directory: false });
  if (!path) return null;
  return { path, graded: await invoke('open_scan', { path }) };
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

/** Compare this scan's identifiers with an earlier saved scan. */
export async function compareWithSaved(scan) {
  requireTauri();
  const path = await open({ filters: SCAN_FILTER, multiple: false, directory: false });
  if (!path) return null;
  return { path, comparison: await invoke('compare_with', { path, scan }) };
}
