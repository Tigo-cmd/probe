<script>
  // The specification at a glance, one tile per part. Missing values say
  // "Not reported"; nothing is filled in.
  import Icon from './Icon.svelte';
  import Meter from './Meter.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import TierMeter from './TierMeter.svelte';
  import { batteryTone, formatBytes, formatGiB, STORAGE_KINDS, tierLabel } from './present.js';

  let { specs, scan } = $props();

  const SPEED = { nvme: 1, sata_ssd: 0.62, unknown: 0.5, hdd: 0.18 };
  const usable = $derived(scan.memory?.total_bytes?.value);
  const sharpness = (h) => (h >= 1440 ? 'Sharp' : h >= 1080 ? 'Full HD' : 'Low resolution');
</script>

<section aria-labelledby="specs-title" class="specs">
  <h2 id="specs-title" class="section-title">At a glance</h2>
  <div class="grid">
    <article class="card tile">
      <div class="head"><span class="badge indigo"><Icon name="chip" size={18} /></span><span class="eyebrow">Processor</span></div>
      <p class="value">{specs.cpu.summary}</p>
      <TierMeter tier={specs.cpu.tier} tone="indigo" />
      <p class="caption faint">{tierLabel(specs.cpu.tier)} performance <ProvenanceTag value="inferred" /></p>
    </article>

    <article class="card tile">
      <div class="head"><span class="badge blue"><Icon name="memory" size={18} /></span><span class="eyebrow">Memory</span></div>
      {#if specs.ram_gb}
        <p class="value big">{specs.ram_gb} GB</p>
        <Meter value={specs.ram_gb / 32} tone="blue" label="{specs.ram_gb} GB of 32 GB" />
        <p class="caption faint">{formatGiB(usable)} usable <ProvenanceTag value="measured" /></p>
      {:else}
        <p class="value missing">Not reported</p>
      {/if}
    </article>

    <article class="card tile">
      <div class="head"><span class="badge teal"><Icon name="drive" size={18} /></span><span class="eyebrow">Storage</span></div>
      {#if specs.storage.kind}
        <p class="value big">{specs.storage.capacity_bytes ? formatBytes(specs.storage.capacity_bytes) : 'Size not reported'}</p>
        <Meter value={SPEED[specs.storage.kind]} tone="teal" label="Speed class: {STORAGE_KINDS[specs.storage.kind]}" />
        <p class="caption faint">{STORAGE_KINDS[specs.storage.kind]} · speed class</p>
      {:else}
        <p class="value missing">No internal drive found</p>
      {/if}
    </article>

    <article class="card tile">
      <div class="head"><span class="badge indigo"><Icon name="graphics" size={18} /></span><span class="eyebrow">Graphics</span></div>
      {#if specs.gpu.name}
        <p class="value">{specs.gpu.name}</p>
        <TierMeter tier={specs.gpu.tier} tone="indigo" />
        <p class="caption faint">
          {tierLabel(specs.gpu.tier)} · {specs.gpu.discrete ? 'dedicated chip' : 'built into the processor'}
          <ProvenanceTag value="inferred" />
        </p>
      {:else}
        <p class="value missing">Not recorded in this scan</p>
      {/if}
    </article>

    <article class="card tile">
      <div class="head"><span class="badge blue"><Icon name="display" size={18} /></span><span class="eyebrow">Screen</span></div>
      {#if specs.display}
        <p class="value big">{specs.display.width} × {specs.display.height}</p>
        <Meter value={Math.min(1, specs.display.height / 1600)} tone="blue" label="{sharpness(specs.display.height)}" />
        <p class="caption faint">{sharpness(specs.display.height)} <ProvenanceTag value="claimed" /></p>
      {:else}
        <p class="value missing">Not reported</p>
      {/if}
    </article>

    <article class="card tile">
      <div class="head"><span class="badge green"><Icon name="battery" size={18} /></span><span class="eyebrow">Battery</span></div>
      {#if specs.battery_percent !== null && specs.battery_percent !== undefined}
        <p class="value big">{specs.battery_percent}%</p>
        <Meter value={specs.battery_percent / 100} tone={batteryTone(specs.battery_percent)} label="{specs.battery_percent}% of original capacity" />
        <p class="caption faint">of its original capacity <ProvenanceTag value="measured" /></p>
      {:else}
        <p class="value missing">{scan.batteries.length ? 'Capacity not reported' : 'No battery detected'}</p>
      {/if}
    </article>
  </div>
</section>

<style>
  .specs {
    display: grid;
    gap: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }
  .tile {
    padding: 18px 20px;
    display: grid;
    gap: 10px;
    align-content: start;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .badge {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: #fff;
  }
  .badge.indigo {
    background: var(--indigo-fill);
  }
  .badge.blue {
    background: var(--blue-fill);
  }
  .badge.teal {
    background: var(--teal-fill);
  }
  .badge.green {
    background: var(--green-fill);
  }
  .value {
    font-weight: 600;
    font-size: 16px;
    line-height: 1.3;
    min-height: 2.6em;
  }
  .value.big {
    font-family: var(--font-display);
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    min-height: 0;
    line-height: 1.25;
  }
  .value.missing {
    color: var(--text-3);
    font-weight: 500;
  }
  .caption {
    font-size: 13px;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: baseline;
  }
  @media (max-width: 860px) {
    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 520px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
