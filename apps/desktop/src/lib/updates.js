// Checking for, downloading and installing new versions of probe.
//
// The app asks GitHub for latest.json from the newest release. Every update
// is signed when it is built and the app refuses one whose signature does
// not match the key built into it, so a tampered download is never run.

import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { demoMode, inTauri } from './api.js';

const SKIPPED_KEY = 'probe.update.skipped';

/** How often a running app looks again, in milliseconds. */
export const RECHECK_MS = 6 * 60 * 60 * 1000;

/**
 * The available update, or null when probe is up to date or the check could
 * not reach GitHub (offline use is normal, so that is not an error).
 * @returns {Promise<{version: string, current: string, notes: string[], install: (onprogress: (p: number|null) => void) => Promise<void>} | null>}
 */
export async function findUpdate() {
  if (import.meta.env.DEV && demoMode) return demoUpdate();
  if (!inTauri) return null;
  let update;
  try {
    update = await check();
  } catch {
    return null;
  }
  if (!update) return null;
  return {
    version: update.version,
    current: update.currentVersion,
    notes: releaseNotes(update.body),
    async install(onprogress) {
      let total = 0;
      let done = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          done += event.data.chunkLength;
          onprogress(total ? Math.min(1, done / total) : null);
        }
        if (event.event === 'Finished') onprogress(1);
      });
      // Windows installers close the app themselves; elsewhere restart it.
      await relaunch();
    },
  };
}

/**
 * The first few readable lines of a release's notes, without Markdown
 * headings, links or the build boilerplate.
 * @param {string | undefined} body
 * @returns {string[]}
 */
export function releaseNotes(body) {
  if (!body) return [];
  return body
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#') && !/^(\*\*)?full changelog/i.test(l))
    .filter((l) => !/code-signed|SmartScreen|SHA256SUMS|right-click the app/i.test(l))
    .map((l) =>
      l
        .replace(/^[-*]\s+/, '')
        .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
        .replace(/\s+by @\S+ in \S+$/, '')
        .replace(/\*\*/g, ''),
    )
    .filter(Boolean)
    .slice(0, 4);
}

/** Whether the person chose "Later" for this exact version. */
export function wasSkipped(version) {
  try {
    return localStorage.getItem(SKIPPED_KEY) === version;
  } catch {
    return false;
  }
}

export function skip(version) {
  try {
    localStorage.setItem(SKIPPED_KEY, version);
  } catch {
    // Storage unavailable: the banner simply comes back next start.
  }
}

function demoUpdate() {
  if (!new URLSearchParams(window.location.search).has('update')) return null;
  return {
    version: '0.3.0',
    current: '0.2.0',
    notes: ['Licence check for Windows and Office', 'Seller mode with a shareable report', 'Faster drive scan'],
    async install(onprogress) {
      for (let p = 0; p <= 1; p += 0.05) {
        onprogress(p);
        await new Promise((r) => setTimeout(r, 120));
      }
      throw new Error('This is a design preview, so nothing was installed.');
    },
  };
}
