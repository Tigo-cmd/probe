<script>
  // One battery: what it holds now against what it held new, and how many
  // charge cycles it has been through against a typical 1000-cycle rating.
  import FieldList from './FieldList.svelte';
  import Meter from './Meter.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import { batteryTone, capacityShare, formatCapacity } from './present.js';

  let { battery: b } = $props();
  const share = $derived(capacityShare(b));
  // An exact match is a firmware default (see the grader): not a reading.
  const exact = $derived(
    b.design_capacity?.value && b.full_charge_capacity?.value?.value === b.design_capacity.value.value,
  );
  const shown = $derived(exact ? null : share);
  const tone = $derived(batteryTone(shown));
  const cycles = $derived(b.cycle_count?.value ?? null);
</script>

<article class="card battery">
  <div class="top">
    <div class="glyph {tone}" role="img" aria-label={shown === null ? 'Capacity not reported' : `Holds ${shown}% of original capacity`}>
      <span class="cell" style="width:{Math.min(100, shown ?? 0)}%"></span>
      <span class="pct">{shown === null ? '–' : `${shown}%`}</span>
    </div>
    <div>
      <p class="eyebrow">Battery {b.name}</p>
      <h3>
        {#if shown === null}
          Capacity not reported
        {:else}
          Holds {shown}% of its original charge
        {/if}
      </h3>
      <p class="faint small">
        {#if exact}Full charge exactly equals design, which is a firmware default rather than a reading.{:else if shown !== null && shown < 80}Below 80%, the point at which Apple considers a battery worn out.{:else if shown !== null}Above 80%, the usual end-of-life line.{/if}
      </p>
    </div>
  </div>

  <div class="charts">
    <div class="chart">
      <p class="label"><span>Now</span><span class="faint">{b.full_charge_capacity?.value ? formatCapacity(b.full_charge_capacity.value) : 'not reported'}</span></p>
      <Meter value={shown === null ? null : Math.min(1, shown / 100)} {tone} height={12} label="Capacity now" />
      <p class="label"><span>When new</span><span class="faint">{b.design_capacity?.value ? formatCapacity(b.design_capacity.value) : 'not reported'}</span></p>
      <Meter value={b.design_capacity?.value ? 1 : null} tone="grey" height={12} label="Design capacity" />
      <p class="faint tiny">From the battery's own fuel gauge <ProvenanceTag value="measured" /></p>
    </div>
    <div class="chart">
      <p class="label"><span>Charge cycles</span><span class="faint">{cycles === null ? 'not reported' : `${cycles.toLocaleString('en-US')} of ~1,000`}</span></p>
      <Meter value={cycles === null ? null : cycles / 1000} tone={cycles > 1000 ? 'amber' : 'blue'} height={12} mark={0.8} label="Charge cycles" />
      <p class="faint tiny">Most packs are rated for 500 to 1,000 full cycles.</p>
    </div>
  </div>

  <details>
    <summary>Battery details</summary>
    <FieldList
      rows={[
        { label: 'Manufacturer', field: b.manufacturer },
        { label: 'Model', field: b.model },
        { label: 'Chemistry', field: b.technology },
        { label: 'Serial', field: b.serial, mono: true },
        { label: 'Made', field: b.manufacture_date },
      ]}
    />
  </details>
</article>

<style>
  .battery {
    padding: 24px;
    display: grid;
    gap: 20px;
  }
  .top {
    display: flex;
    gap: 20px;
    align-items: center;
  }
  h3 {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .glyph {
    position: relative;
    flex: none;
    width: 108px;
    height: 52px;
    border-radius: 12px;
    border: 3px solid var(--text-3);
    padding: 4px;
    display: flex;
  }
  .glyph::after {
    content: '';
    position: absolute;
    right: -10px;
    top: 15px;
    width: 5px;
    height: 16px;
    border-radius: 0 4px 4px 0;
    background: var(--text-3);
  }
  .cell {
    border-radius: 6px;
    background: var(--grey-fill);
    transition: width 900ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .green .cell {
    background: var(--green-fill);
  }
  .amber .cell {
    background: var(--amber-fill);
  }
  .red .cell {
    background: var(--red-fill);
  }
  .pct {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 17px;
    color: var(--text);
    text-shadow: 0 0 6px var(--surface);
  }
  .charts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }
  .chart {
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .label {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    font-weight: 500;
  }
  details summary {
    cursor: pointer;
    color: var(--accent);
    font-size: 14px;
    margin-bottom: 10px;
  }
  @media (max-width: 620px) {
    .charts {
      grid-template-columns: 1fr;
    }
    .top {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
