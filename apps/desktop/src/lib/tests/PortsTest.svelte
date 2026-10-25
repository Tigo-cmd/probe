<script>
  // Plug a USB device into each port in turn. probe polls the system's USB
  // device list and marks a port working the moment something new appears,
  // then waits for it to be unplugged before the next port.
  import { onDestroy } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { liveUsb } from '../api.js';
  import { newDevices, portsResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  let count = $state(null);
  let ports = $state([]);
  let names = $state([]);
  let current = $state(0);
  let phase = $state('plug'); // plug | unplug
  let waited = $state(0);
  let baseline = [];
  let seen = null;
  let timer;

  async function begin(n) {
    count = n;
    ports = Array(n).fill(null);
    baseline = await liveUsb().catch(() => []);
    poll();
  }

  function poll() {
    clearInterval(timer);
    waited = 0;
    timer = setInterval(async () => {
      waited++;
      const now = await liveUsb().catch(() => null);
      if (!now) return;
      if (phase === 'plug') {
        const fresh = newDevices(baseline, now);
        if (fresh.length) {
          seen = fresh[0];
          names = [...names, fresh[0].name];
          mark('ok');
          phase = 'unplug';
          waited = 0;
        }
      } else if (seen && !now.some((d) => d.id === seen.id)) {
        baseline = now;
        seen = null;
        phase = 'plug';
        waited = 0;
        if (current >= count) finish();
      }
    }, 1000);
  }

  function mark(state) {
    ports[current] = state;
    current++;
    if (state !== 'ok') {
      phase = 'plug';
      if (current >= count) finish();
    }
  }

  function finish() {
    clearInterval(timer);
    oncomplete(portsResult(ports.map((p) => p ?? 'skipped'), names));
  }

  onDestroy(() => clearInterval(timer));
</script>

<TestFrame eyebrow="USB ports" title={count === null ? 'How many USB ports does it have?' : current < count ? `Port ${current + 1} of ${count}` : 'All ports checked'} lede={count === null ? 'Count both the rectangular USB-A ports and the small oval USB-C ports. Have a USB stick, mouse or phone cable ready.' : ''}>
  {#if count === null}
    <div class="counts">
      {#each [1, 2, 3, 4, 5, 6] as n (n)}
        <button class="count" onclick={() => begin(n)}>{n}</button>
      {/each}
    </div>
  {:else}
    <div class="panel">
      <div class="ports">
        {#each ports as p, i (i)}
          <div class="port {p ?? (i === current ? 'now' : '')}">
            <Icon name={p === 'ok' ? 'check' : p === 'failed' ? 'cross' : 'usb'} size={22} stroke={2.2} />
            <span>Port {i + 1}</span>
          </div>
        {/each}
      </div>
      {#if current < count}
        <p class="status-line {phase === 'unplug' ? 'ok' : 'wait'}">
          {#if phase === 'plug'}
            <span class="spin"></span> Plug a USB device into port {current + 1}…
          {:else}
            <Icon name="check" size={18} /> Detected {names.at(-1)}. Now unplug it.
          {/if}
        </p>
      {/if}
    </div>
  {/if}
  {#snippet footer()}
    {#if count !== null && current < count && phase === 'plug'}
      {#if waited >= 10}<button onclick={() => mark('failed')}>Nothing happens</button>{/if}
      <button class="plain" onclick={() => mark('skipped')}>Skip this port</button>
    {/if}
    {#if count !== null && phase === 'unplug'}
      <button class="plain" onclick={() => { phase = 'plug'; if (current >= count) finish(); }}>Continue without unplugging</button>
    {/if}
    <button class="plain" onclick={oncancel}>Cancel</button>
  {/snippet}
</TestFrame>

<style>
  .counts {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .count {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    font-size: 22px;
    font-weight: 700;
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .panel {
    display: grid;
    gap: 18px;
  }
  .ports {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .port {
    display: grid;
    justify-items: center;
    gap: 6px;
    width: 92px;
    padding: 14px 8px;
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--text-3);
    font-size: 13px;
    font-weight: 600;
  }
  .port.now {
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--accent);
  }
  .port.ok {
    background: var(--green-bg);
    color: var(--green);
  }
  .port.failed {
    background: var(--red-bg);
    color: var(--red);
  }
</style>
