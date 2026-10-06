// The report must never say "healthy" or "not stolen": a clean result lowers
// risk, it does not clear a machine, and theft cannot be determined at all.
// The core crate enforces this for the text report; this does it for the UI.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { expect, it } from 'vitest';

function sources(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return name === 'fixtures' ? [] : sources(p);
    return /\.(svelte|js)$/.test(name) && !name.endsWith('.test.js') ? [p] : [];
  });
}

it('no UI source says healthy or not stolen', () => {
  const files = sources(new URL('.', import.meta.url).pathname);
  expect(files.length).toBeGreaterThan(5);
  for (const f of files) {
    const text = readFileSync(f, 'utf8').toLowerCase();
    expect(text, f).not.toContain('healthy');
    expect(text, f).not.toContain('not stolen');
  }
});
