<script>
  // Every core works flat out for 45 seconds. A cool laptop keeps its pace;
  // one with a clogged fan or dried thermal paste slows as it heats up.
  // The pace is measured directly, so this works even where the system
  // reports no temperature; temperature and clock are shown where it does.
  import { onDestroy } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { liveCpu } from '../api.js';
  import { performanceResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();
  const SECONDS = 45;
  const cores = Math.max(1, Math.min(32, navigator.hardwareConcurrency || 4));

  let phase = $state('intro');
  let perSecond = $state([]);
  let cpu = $state(null);
  let temps = [];
  let limits = [];
  let workers = [];
  let bucket = 0;
  let timers = [];

  function start() {
    phase = 'running';
    perSecond = [];
    temps = [];
    limits = [];
    for (let i = 0; i < cores; i++) {
      const w = new Worker(new URL('./loadWorker.js', import.meta.url), { type: 'module' });
      w.onmessage = (e) => (bucket += e.data.units);
      w.postMessage('start');
      workers.push(w);
    }
    timers.push(
      setInterval(() => {
        perSecond = [...perSecond, bucket];
        bucket = 0;
        if (perSecond.length >= SECONDS) finish();
      }, 1000),
    );
    timers.push(
      setInterval(async () => {
        cpu = await liveCpu().catch(() => null);
        if (cpu?.temp_c != null) temps.push(cpu.temp_c);
        if (cpu?.limit_pct != null) limits.push(cpu.limit_pct);
      }, 2000),
    );
  }

  function stopAll() {
    timers.forEach(clearInterval);
    timers = [];
    workers.forEach((w) => {
      w.postMessage('stop');
      w.terminate();
    });
    workers = [];
  }

  function finish() {
    stopAll();
    oncomplete(performanceResult(perSecond, temps, limits, cores));
  }

  onDestroy(stopAll);

  const peak = $derived(Math.max(1, ...perSecond));
</script>

<TestFrame eyebrow="Performance under load" title={phase === 'intro' ? 'Does it keep its speed when hot?' : `Working all ${cores} cores… ${SECONDS - perSecond.length} s`} lede={phase === 'intro' ? `For ${SECONDS} seconds every core works flat out. The fan will speed up; that is expected. Plug the charger in for a fair result.` : ''} wide>
  {#if phase === 'running'}
    <div class="panel">
      <div class="chart" aria-label="Work done each second">
        {#each Array(SECONDS) as _, i (i)}
          <span style="height:{perSecond[i] ? (perSecond[i] / peak) * 100 : 0}%"></span>
        {/each}
      </div>
      <div class="stat-row">
        <div class="stat"><span class="faint small">Pace now</span><b>{perSecond.length ? Math.round((perSecond.at(-1) / peak) * 100) : '–'}%</b></div>
        <div class="stat"><span class="faint small">Temperature</span><b>{cpu?.temp_c != null ? `${Math.round(cpu.temp_c)} °C` : 'Not reported'}</b></div>
        <div class="stat"><span class="faint small">Clock</span><b>{cpu?.mhz ? `${(cpu.mhz / 1000).toFixed(1)} GHz` : 'Not reported'}</b></div>
        {#if cpu?.limit_pct != null}<div class="stat"><span class="faint small">Speed limit</span><b>{cpu.limit_pct}%</b></div>{/if}
      </div>
    </div>
  {/if}
  {#snippet footer()}
    {#if phase === 'intro'}
      <button class="primary" onclick={start}><Icon name="gauge" size={16} /> Start</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    {:else}
      <button onclick={finish}>Stop now</button>
    {/if}
  {/snippet}
</TestFrame>

<style>
  .panel {
    display: grid;
    gap: 18px;
  }
  .chart {
    height: 160px;
    display: flex;
    align-items: flex-end;
    gap: 3px;
  }
  .chart span {
    flex: 1;
    border-radius: 4px 4px 0 0;
    background: var(--indigo-fill);
    transition: height 300ms;
  }
</style>
