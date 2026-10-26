<script>
  // Unplugged, the laptop runs at light load (this screen, kept awake) while
  // probe samples the battery every ten seconds, then estimates how long a
  // full charge lasts. Ten minutes gives a steady figure; three is the least.
  import { onDestroy, onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { livePower } from '../api.js';
  import { batteryResult, drainEstimate, hoursText } from '../handson.js';

  let { oncomplete, oncancel } = $props();
  const TARGET_MIN = 10;

  let power = $state(null);
  let phase = $state('ready'); // ready | running
  let samples = $state([]);
  let started = 0;
  let elapsed = $state(0);
  let timer;
  let clock;
  let wake = null;

  onMount(async () => {
    power = await livePower().catch(() => null);
    timer = setInterval(async () => {
      if (phase !== 'ready') return;
      power = await livePower().catch(() => null);
    }, 2000);
  });

  async function start() {
    phase = 'running';
    started = performance.now();
    samples = [];
    // Keep the screen on so the test measures a laptop in use, not asleep.
    try {
      wake = await navigator.wakeLock?.request('screen');
    } catch {
      wake = null;
    }
    await sample();
    clearInterval(timer);
    timer = setInterval(sample, 10_000);
    clock = setInterval(() => {
      elapsed = (performance.now() - started) / 1000;
      if (elapsed >= TARGET_MIN * 60) finish();
    }, 1000);
  }

  async function sample() {
    const p = await livePower().catch(() => null);
    if (!p) return;
    power = p;
    if (p.on_ac) return; // plugged in mid-test: those samples do not count
    samples = [...samples, { t: (performance.now() - started) / 1000, energy_mwh: p.energy_mwh, percent: p.percent, rate_mw: p.rate_mw }];
  }

  function stop() {
    clearInterval(timer);
    clearInterval(clock);
    wake?.release?.();
  }
  function finish() {
    stop();
    oncomplete(batteryResult(samples, power?.full_mwh ?? null));
  }
  onDestroy(stop);

  const estimate = $derived(drainEstimate(samples, power?.full_mwh ?? null));
  const mmss = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
</script>

<TestFrame eyebrow="Battery drain" title={phase === 'ready' ? 'How long does the battery really last?' : `Measuring… ${mmss(elapsed)} of ${TARGET_MIN}:00`} lede={phase === 'ready' ? `Unplug the charger and leave the computer on this screen for ${TARGET_MIN} minutes. probe measures how fast the battery empties at light use. You can stop after three minutes, but longer is more accurate.` : ''}>
  <div class="panel">
    {#if power?.on_ac && phase === 'ready'}
      <p class="status-line wait"><Icon name="plug" size={18} /> Unplug the charger to start.</p>
    {:else if power?.on_ac}
      <p class="status-line bad"><Icon name="plug" size={18} /> The charger is plugged in: unplug it to keep measuring.</p>
    {/if}
    {#if power && power.on_ac == null && power.percent == null}
      <p class="status-line bad">This computer does not report its battery here.</p>
    {/if}
    <div class="stat-row">
      <div class="stat"><span class="faint small">Charge</span><b>{power?.percent != null ? `${Math.round(power.percent)}%` : '–'}</b></div>
      <div class="stat"><span class="faint small">Drawing</span><b>{estimate?.watts ? `${estimate.watts.toFixed(1)} W` : power?.rate_mw && power.rate_mw < 0 ? `${(-power.rate_mw / 1000).toFixed(1)} W` : '–'}</b></div>
      <div class="stat"><span class="faint small">Full charge lasts</span><b>{estimate?.runtime_h ? hoursText(estimate.runtime_h) : phase === 'running' ? 'Measuring…' : '–'}</b></div>
    </div>
    {#if phase === 'running'}
      <div class="progress"><span style="width:{Math.min(100, (elapsed / (TARGET_MIN * 60)) * 100)}%"></span></div>
    {/if}
  </div>
  {#snippet footer()}
    {#if phase === 'ready'}
      <button class="primary" onclick={start} disabled={!power || power.on_ac !== false}><Icon name="battery" size={16} /> Start</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    {:else}
      <button onclick={finish} disabled={elapsed < 180}>{elapsed < 180 ? `Stop available in ${mmss(180 - elapsed)}` : 'Stop and save'}</button>
      <button class="plain" onclick={() => { stop(); oncancel(); }}>Cancel</button>
    {/if}
  {/snippet}
</TestFrame>

<style>
  .panel {
    display: grid;
    gap: 16px;
  }
  .progress {
    height: 10px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--green-fill);
    transition: width 900ms linear;
  }
</style>
