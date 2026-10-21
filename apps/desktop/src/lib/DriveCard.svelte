<script>
  // One drive: wear left as a ring when the drive reports it, the error
  // counters that predict failure, and how much it has been used.
  import FieldList from './FieldList.svelte';
  import Icon from './Icon.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import Ring from './Ring.svelte';
  import { ataCount, formatBytes, nvmeWritten, powerOnText, SMART_NAMES, TRANSPORTS } from './present.js';

  let { drive: d } = $props();
  const h = $derived(d.health);
  const life = $derived(h.kind === 'nvme' ? Math.max(0, 100 - Math.min(100, h.percentage_used)) : null);
  const lifeTone = $derived(life === null ? 'grey' : life >= 20 ? 'green' : 'amber');
  const external = $derived(d.transport === 'usb' || d.removable);

  /** Counters where any value above zero is a warning sign. */
  const counters = $derived(
    h.kind === 'nvme'
      ? [
          { label: 'Media errors', value: Number(h.media_errors), bad: Number(h.media_errors) > 0 },
          { label: 'Critical warning', value: h.critical_warning, bad: h.critical_warning !== 0, text: h.critical_warning ? `0x${h.critical_warning.toString(16)}` : 'None' },
          { label: 'Spare blocks', value: h.available_spare, bad: h.available_spare_threshold > 0 && h.available_spare <= h.available_spare_threshold, text: `${h.available_spare}%` },
        ]
      : h.kind === 'ata'
        ? [5, 187, 188, 197, 198]
            .map((id) => h.attributes.find((a) => a.id === id))
            .filter(Boolean)
            .map((a) => ({ label: SMART_NAMES[a.id], value: ataCount(a), bad: ataCount(a) > 0 }))
        : [],
  );
</script>

<article class="card drive">
  <div class="top">
    <Ring value={life} size={96} stroke={10} tone={lifeTone} label={life === null ? 'Wear not reported' : `${life}% of rated life left`}>
      {#if life !== null}
        <span class="ring-num">{life}%</span>
      {:else}
        <Icon name="drive" size={30} />
      {/if}
    </Ring>
    <div class="title">
      <p class="eyebrow">{TRANSPORTS[d.transport] ?? d.transport}{external ? ' · external' : ''} · {d.name}</p>
      <h3>{d.model?.value ?? 'Drive'}</h3>
      <p class="faint small">
        {d.capacity_bytes?.value ? formatBytes(d.capacity_bytes.value) : 'Size not reported'}
        {#if life !== null}· about {life}% of rated life left (vendor estimate){/if}
      </p>
    </div>
  </div>

  {#if h.kind === 'unavailable'}
    <p class="notice"><Icon name="question" size={16} /> Health not read: {h.reason}</p>
  {:else}
    <div class="stats">
      {#each counters as c (c.label)}
        <div class="stat" class:bad={c.bad}>
          <span class="stat-num">{c.text ?? c.value.toLocaleString('en-US')}</span>
          <span class="faint tiny">{c.label}</span>
        </div>
      {/each}
    </div>
    {#if h.kind === 'nvme'}
      <dl class="usage">
        <dt>Written</dt>
        <dd>{nvmeWritten(h.data_units_written)} <ProvenanceTag value="measured" /></dd>
        <dt>Powered on</dt>
        <dd>{powerOnText(h.power_on_hours)}</dd>
        <dt>Power cycles</dt>
        <dd>{Number(h.power_cycles).toLocaleString('en-US')}, of which {Number(h.unsafe_shutdowns).toLocaleString('en-US')} unsafe shutdowns</dd>
      </dl>
    {/if}
  {/if}

  <details>
    <summary>Drive details</summary>
    <FieldList
      rows={[
        { label: 'Model', field: d.model },
        { label: 'Serial', field: d.serial, mono: true },
        { label: 'Firmware', field: d.firmware, mono: true },
        { label: 'Capacity', field: d.capacity_bytes, format: formatBytes },
        { label: 'Spinning disk', field: d.rotational, format: (v) => (v ? 'Yes' : 'No') },
      ]}
    />
  </details>
</article>

<style>
  .drive {
    padding: 24px;
    display: grid;
    gap: 18px;
  }
  .top {
    display: flex;
    gap: 20px;
    align-items: center;
  }
  .title {
    min-width: 0;
  }
  h3 {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .ring-num {
    font-weight: 700;
    font-size: 20px;
    letter-spacing: -0.02em;
  }
  .notice {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 14px;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 10px;
  }
  .stat {
    padding: 12px 14px;
    border-radius: 14px;
    background: var(--surface-2);
    display: grid;
    gap: 2px;
  }
  .stat-num {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--green);
  }
  .stat.bad {
    background: var(--red-bg);
  }
  .stat.bad .stat-num {
    color: var(--red);
  }
  .usage {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 18px;
    margin: 0;
    font-size: 14px;
  }
  .usage dt {
    color: var(--text-3);
  }
  .usage dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: baseline;
  }
  details summary {
    cursor: pointer;
    color: var(--accent);
    font-size: 14px;
    margin-bottom: 10px;
  }
  @media (max-width: 520px) {
    .top {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
