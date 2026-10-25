<script>
  // Finds the Wi-Fi and Bluetooth radios and whether they are on; the
  // tester confirms Wi-Fi can actually see networks around them.
  import { onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { liveRadios } from '../api.js';
  import { wirelessResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();
  let radios = $state(null);
  let error = $state('');
  let sees = $state(null);

  async function check() {
    radios = null;
    error = '';
    try {
      radios = await liveRadios();
    } catch (e) {
      error = String(e);
      radios = [];
    }
  }
  onMount(check);

  const wifi = $derived((radios ?? []).filter((r) => r.kind === 'wifi'));
  const bt = $derived((radios ?? []).filter((r) => r.kind === 'bluetooth'));
  const radioState = (r) => (r.enabled === false ? 'Switched off' : r.kind === 'wifi' ? (r.connected ? 'On, connected' : 'On, not connected') : r.enabled ? 'On' : 'Found');
</script>

<TestFrame eyebrow="Wi-Fi and Bluetooth" title="Are the radios working?" lede="probe looks for the Wi-Fi and Bluetooth hardware and whether it is switched on. Then check Wi-Fi can see the networks around you.">
  <div class="panel list">
    {#if radios === null}
      <p class="status-line wait"><span class="spin"></span> Looking for radios…</p>
    {:else}
      {#each [['Wi-Fi', 'wifi', wifi], ['Bluetooth', 'bluetooth', bt]] as [label, icon, found] (label)}
        <div class="row">
          <Icon name={icon} size={20} />
          <div class="name">
            <b>{label}</b>
            <span class="faint small">{found.length ? found.map((r) => r.name).join(', ') : 'Not found'}</span>
          </div>
          {#if found.length}
            <span class={found.some((r) => r.enabled === false) ? 'bad' : 'ok'}>{radioState(found[0])}</span>
          {:else}
            <span class="bad">Missing</span>
          {/if}
        </div>
      {/each}
      {#if error}<p class="small bad">{error}</p>{/if}
    {/if}
  </div>
  {#if wifi.length}
    <div class="panel ask">
      <p>Open the Wi-Fi menu of this laptop. Can you see nearby networks?</p>
      <div class="choice">
        <button class:primary={sees === true} onclick={() => (sees = true)}>Yes</button>
        <button class:primary={sees === false} onclick={() => (sees = false)}>No networks</button>
      </div>
    </div>
  {/if}
  {#snippet footer()}
    <button class="primary" disabled={radios === null} onclick={() => oncomplete(wirelessResult(radios, sees))}>Done</button>
    <button onclick={check}>Check again</button>
    <button class="plain" onclick={oncancel}>Cancel</button>
  {/snippet}
</TestFrame>

<style>
  .list {
    display: grid;
    gap: 14px;
  }
  .row {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .name {
    flex: 1;
    display: grid;
  }
  .row > span {
    font-weight: 600;
    font-size: 14px;
  }
  .ask {
    display: grid;
    gap: 10px;
  }
  .choice {
    display: flex;
    gap: 8px;
  }
</style>
