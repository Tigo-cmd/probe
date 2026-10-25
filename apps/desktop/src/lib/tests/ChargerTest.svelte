<script>
  // Unplug, then plug in again: probe watches the system's power status to
  // confirm the charger is detected and the battery takes charge, and reads
  // the charging rate and the charger's rating where the system reports them.
  import { onDestroy, onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { livePower } from '../api.js';
  import { chargerResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  let power = $state(null);
  let phase = $state('checking'); // checking | unplug | plug | watch | unsupported
  let seen = $state({ unplugged: false, plugged: false, charging: false, rate_w: null, adapter_w: null, percent: null });
  let watched = $state(0);
  let waited = $state(0);
  let timer;

  onMount(() => {
    timer = setInterval(tick, 1000);
    tick();
  });
  onDestroy(() => clearInterval(timer));

  async function tick() {
    power = await livePower().catch(() => null);
    if (!power) return;
    if (phase === 'checking') {
      phase = power.on_ac == null ? 'unsupported' : power.on_ac ? 'unplug' : 'plug';
      if (phase === 'plug') seen.unplugged = true;
      return;
    }
    waited++;
    if (phase === 'unplug' && power.on_ac === false) {
      seen.unplugged = true;
      phase = 'plug';
      waited = 0;
    } else if (phase === 'plug' && power.on_ac === true) {
      seen.plugged = true;
      phase = 'watch';
      waited = 0;
    } else if (phase === 'watch') {
      watched++;
      seen.percent = power.percent;
      if (power.charging) seen.charging = true;
      if (power.rate_mw != null && power.rate_mw > 0) seen.rate_w = Math.round(power.rate_mw / 1000);
      if (power.adapter_watts) seen.adapter_w = power.adapter_watts;
      // Some laptops take a few seconds to start charging.
      if (watched >= 12 || (seen.charging && seen.rate_w !== null && watched >= 5)) {
        clearInterval(timer);
        phase = 'done';
      }
    }
  }

  const title = $derived(
    {
      checking: 'Checking power…',
      unplug: 'Unplug the charger',
      plug: 'Now plug the charger in',
      watch: 'Watching it charge…',
      done: seen.charging ? 'Charging' : 'Not charging',
      unsupported: 'This computer does not report charging',
    }[phase],
  );
</script>

<TestFrame eyebrow="Charger" {title} lede={phase === 'unsupported' ? 'The system gives no charger status here, so this test cannot run. Check by hand that the charging light comes on.' : 'Use the charger that comes with the laptop. probe watches for it to be detected and for the battery to take charge.'}>
  <div class="panel">
    <div class="steps">
      <div class="step {seen.unplugged ? 'ok' : phase === 'unplug' ? 'now' : ''}"><Icon name={seen.unplugged ? 'check' : 'plug'} size={18} /> Unplugged</div>
      <div class="step {seen.plugged ? 'ok' : phase === 'plug' ? 'now' : ''}"><Icon name={seen.plugged ? 'check' : 'plug'} size={18} /> Plugged in</div>
      <div class="step {seen.charging ? 'ok' : phase === 'watch' ? 'now' : phase === 'done' ? 'bad' : ''}"><Icon name={seen.charging ? 'check' : 'battery'} size={18} /> Charging</div>
    </div>
    {#if power}
      <div class="stat-row">
        <div class="stat"><span class="faint small">Battery</span><b>{power.percent != null ? `${Math.round(power.percent)}%` : '–'}</b></div>
        <div class="stat"><span class="faint small">Charging rate</span><b>{power.rate_mw != null && power.rate_mw > 0 ? `${Math.round(power.rate_mw / 1000)} W` : '–'}</b></div>
        <div class="stat"><span class="faint small">Charger rating</span><b>{power.adapter_watts ? `${power.adapter_watts} W` : 'Not reported'}</b></div>
      </div>
    {/if}
  </div>
  {#snippet footer()}
    {#if phase === 'done'}
      <button class="primary" onclick={() => oncomplete(chargerResult(seen))}>Save result</button>
    {:else if (phase === 'plug' || phase === 'unplug') && waited > 45}
      <button onclick={() => { clearInterval(timer); oncomplete(chargerResult(seen)); }}>It is not detected</button>
    {/if}
    <button class="plain" onclick={oncancel}>Cancel</button>
  {/snippet}
</TestFrame>

<style>
  .panel {
    display: grid;
    gap: 18px;
  }
  .steps {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .step {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text-3);
    font-weight: 600;
    font-size: 14px;
  }
  .step.now {
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--accent);
  }
  .step.ok {
    background: var(--green-bg);
    color: var(--green);
  }
  .step.bad {
    background: var(--red-bg);
    color: var(--red);
  }
</style>
