import { describe, expect, it } from 'vitest';
import { releaseNotes } from './updates.js';

describe('releaseNotes', () => {
  it('keeps the first readable lines of GitHub release notes', () => {
    const body = [
      "## What's Changed",
      '* Save reports as a web page by @Tigo-cmd in https://github.com/Tigo-cmd/probe/pull/4',
      '- **Desktops** no longer lose points for having no battery',
      'These builds are not code-signed yet. Windows SmartScreen needs',
      '**Full Changelog**: https://github.com/Tigo-cmd/probe/compare/v0.1.0...v0.2.0',
    ].join('\n');
    expect(releaseNotes(body)).toEqual([
      'Save reports as a web page',
      'Desktops no longer lose points for having no battery',
    ]);
  });

  it('copes with no notes', () => {
    expect(releaseNotes(undefined)).toEqual([]);
    expect(releaseNotes('')).toEqual([]);
  });
});
