<script>
  // Fills free memory with known patterns and reads them back. Two workers
  // share the work so it finishes in a minute or two.
  import { onDestroy } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { memoryResult, memoryTarget } from '../handson.js';

  let { oncomplete, oncancel, memoryBytes = null } = $props();
  const WORKERS = 2;
  const target = memoryTarget(memoryBytes);
  const totalMb = memoryBytes ? Math.round(memoryBytes / (1024 * 1024)) : null;

  let phase = $state('intro');
  let progress = $state(0);
  let errors = $state(0);
  let grantedMb = $state(0);
  let workers = [];
  let shares = [];

  function start() {
    phase = 'running';
    shares = Array.from({ length: WORKERS }, () => ({ done: 0, total: 1, errors: 0, examples: [], mb: 0, finished: false }));
    for (let n = 0; n < WORKERS; n++) {
      const w = new Worker(new URL('./memWorker.js', import.meta.url), { type: 'module' });
      w.onmessage = (e) => onMessage(n, e.data);
      w.onerror = () => onMessage(n, { type: 'done', errors: 0, examples: [], mb: 0 });
      w.postMessage({ mb: target / WORKERS, seed: (Math.random() * 2 ** 32) >>> 0 });
      workers.push(w);
    }
  }

  function onMessage(n, m) {
    const s = shares[n];
    if (m.type === 'granted') s.mb = m.mb;
    if (m.type === 'progress') Object.assign(s, { done: m.done, total: m.total, errors: m.errors });
    if (m.type === 'done' || m.type === 'stopped') Object.assign(s, { errors: m.errors, examples: m.examples, finished: m.type === 'done' });
    grantedMb = shares.reduce((a, x) => a + x.mb, 0);
    errors = shares.reduce((a, x) => a + x.errors, 0);
    progress = shares.reduce((a, x) => a + x.done / x.total, 0) / WORKERS;
    if (m.type === 'done' && shares.every((x) => x.finished)) finish(true);
  }

  function stopAll() {
    workers.forEach((w) => {
      w.postMessage('stop');
      w.terminate();
    });
    workers = [];
  }

  function finish(finished) {
    stopAll();
    oncomplete(
      memoryResult({
        testedMb: grantedMb,
        totalMb,
        errors,
        examples: shares.flatMap((s) => s.examples).slice(0, 5),
        finished,
      }),
    );
  }

  onDestroy(stopAll);
</script>

<TestFrame
  eyebrow="Memory (RAM)"
  title={phase === 'intro' ? 'Is the memory reliable?' : `Checking ${(grantedMb / 1024).toFixed(1)} GB… ${Math.round(progress * 100)}%`}
  lede={phase === 'intro'
    ? `probe fills about ${(target / 1024).toFixed(1)} GB of free memory with patterns and reads every value back. It takes a minute or two; close other apps first. This checks the memory the system lends to probe, not every cell: for a full test, run MemTest86 from a USB stick.`
    : ''}
>
  {#if phase === 'running'}
    <div class="panel">
      <div class="bar"><i style="width:{Math.round(progress * 100)}%"></i></div>
      <div class="stat-row">
        <div class="stat"><span class="faint small">Memory under test</span><b>{(grantedMb / 1024).toFixed(1)} GB</b></div>
        <div class="stat"><span class="faint small">Errors</span><b class:bad={errors > 0}>{errors.toLocaleString()}</b></div>
      </div>
    </div>
  {/if}
  {#snippet footer()}
    {#if phase === 'intro'}
      <button class="primary" onclick={start}><Icon name="memory" size={16} /> Start</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    {:else}
      <button onclick={() => finish(false)}>Stop now</button>
    {/if}
  {/snippet}
</TestFrame>

<style>
  .panel {
    display: grid;
    gap: 18px;
  }
  .bar {
    height: 10px;
    border-radius: 99px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--indigo-fill, var(--blue-fill));
    transition: width 300ms;
  }
  .bad {
    color: var(--red);
  }
</style>
