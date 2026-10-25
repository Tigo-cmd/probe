<script>
  // Read-only drive speed: probe reads existing system files with the
  // operating system's cache bypassed and times it. Nothing is written.
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { readSpeed } from '../api.js';
  import { READ_FLOORS, storageResult } from '../handson.js';
  import { STORAGE_KINDS } from '../present.js';

  let { kind = null, oncomplete, oncancel } = $props();
  let running = $state(false);
  let speed = $state(null);
  let error = $state('');

  async function run() {
    running = true;
    error = '';
    try {
      speed = await readSpeed();
    } catch (e) {
      error = String(e);
    }
    running = false;
  }
  const res = $derived(speed ? storageResult(speed, kind) : null);
  const scale = 3500; // MB/s at the right end of the bar
</script>

<TestFrame eyebrow="Drive read speed" title="How fast does the drive read?" lede="Reads about a gigabyte of existing system files with the cache bypassed, and times it. Nothing is written to the drive.">
  <div class="panel">
    {#if running}
      <p class="status-line wait"><span class="spin"></span> Reading…</p>
    {:else if speed}
      <p class="big">{Math.round(speed.mb_per_s).toLocaleString('en-US')} <span class="unit">MB/s</span></p>
      <div class="bar"><span class={res.outcome === 'passed' ? 'okfill' : 'badfill'} style="width:{Math.min(100, (speed.mb_per_s / scale) * 100)}%"></span>
        {#if READ_FLOORS[kind]}<i style="left:{(READ_FLOORS[kind] / scale) * 100}%" title="Minimum expected"></i>{/if}
      </div>
      <p class="small {res.outcome === 'passed' ? 'ok' : 'bad'}">{res.summary}</p>
      <p class="faint small">{(speed.bytes / 1e6).toFixed(0)} MB from {speed.files} files in {speed.seconds.toFixed(1)} s · {speed.method}</p>
    {:else}
      <p class="muted">Main drive: {STORAGE_KINDS[kind] ?? 'type not known'}. Takes up to 15 seconds.</p>
    {/if}
    {#if error}<p class="bad small">{error}</p>{/if}
  </div>
  {#snippet footer()}
    {#if speed}
      <button class="primary" onclick={() => oncomplete(res)}>Save result</button>
      <button onclick={run} disabled={running}>Run again</button>
    {:else}
      <button class="primary" onclick={run} disabled={running}><Icon name="drive" size={16} /> Start</button>
    {/if}
    <button class="plain" onclick={oncancel}>Cancel</button>
  {/snippet}
</TestFrame>

<style>
  .panel {
    display: grid;
    gap: 12px;
  }
  .big {
    font-family: var(--font-display);
    font-size: 56px;
    font-weight: 700;
    letter-spacing: -0.03em;
    line-height: 1;
  }
  .unit {
    font-size: 22px;
    color: var(--text-3);
  }
  .bar {
    position: relative;
    height: 14px;
    border-radius: 999px;
    background: var(--track);
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 999px;
  }
  .okfill {
    background: var(--teal-fill);
  }
  .badfill {
    background: var(--red-fill);
  }
  .bar i {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 2px;
    background: var(--text-3);
  }
</style>
