<script>
  // "What will you use it for?" The buyer picks a use; the card shows the
  // Rust assessment's verdict for it with its reasons, and a bar chart ranks
  // every use side by side. Nothing is scored here: this only presents.
  import Icon from './Icon.svelte';
  import Meter from './Meter.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import Ring from './Ring.svelte';
  import { fit, rankUses, useCase } from './present.js';

  let { uses, selected = $bindable() } = $props();

  const current = $derived(uses.find((u) => u.use_case === selected) ?? uses[0]);
  const meta = $derived(useCase(current.use_case));
  const verdict = $derived(fit(current.fit));
  const ranked = $derived(rankUses(uses));

  const REASON_ICON = { good: 'check', fair: 'caution', bad: 'cross' };

  /** Arrow keys move through the picker, as in a native segmented control. */
  function onkey(e) {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const i = uses.findIndex((u) => u.use_case === current.use_case);
    selected = uses[(i + step + uses.length) % uses.length].use_case;
    e.currentTarget.querySelector('[aria-checked="true"]')?.focus();
  }
</script>

<section class="card usefit" aria-labelledby="usefit-title">
  <header>
    <h2 id="usefit-title" class="section-title">What will you use it for?</h2>
    <p class="faint">Pick one to see how well this computer suits it.</p>
  </header>

  <div class="picker" role="radiogroup" aria-label="Intended use" tabindex="-1" onkeydown={onkey}>
    {#each uses as u (u.use_case)}
      {@const m = useCase(u.use_case)}
      <button
        role="radio"
        aria-checked={u.use_case === current.use_case}
        tabindex={u.use_case === current.use_case ? 0 : -1}
        class:selected={u.use_case === current.use_case}
        onclick={() => (selected = u.use_case)}
      >
        <Icon name={m.icon} size={22} />
        <span>{m.label}</span>
      </button>
    {/each}
  </div>

  <div class="body">
    <div class="detail" aria-live="polite">
      <div class="verdict">
        <Ring value={current.score} size={104} stroke={11} tone={verdict.tone} label="{meta.title}: {verdict.label}">
          <span class="ring-num">{current.score ?? '–'}</span>
        </Ring>
        <div class="verdict-text">
          <p class="eyebrow">{meta.title}</p>
          <h3 class="tone-{verdict.tone}">{verdict.label}</h3>
          <p class="faint small">{meta.blurb}</p>
        </div>
      </div>

      {#if current.reasons.length}
        <ul class="reasons">
          {#each current.reasons as r, i (i)}
            <li>
              <span class="dot {r.tone}"><Icon name={REASON_ICON[r.tone] ?? 'info'} size={14} stroke={2.4} /></span>
              <span class="reason-text">{r.text} <ProvenanceTag value={r.provenance} /></span>
            </li>
          {/each}
        </ul>
      {/if}
      {#if current.unknowns.length}
        <p class="unknowns faint small">
          <Icon name="question" size={15} />
          <span><b>Not checked:</b> {current.unknowns.join(', ').toLowerCase()}. These are left out of the rating, not assumed fine.</span>
        </p>
      {/if}
    </div>

    <div class="chart">
      <p class="eyebrow">How it suits each use</p>
      <ul>
        {#each ranked as u (u.use_case)}
          {@const m = useCase(u.use_case)}
          {@const f = fit(u.fit)}
          <li>
            <button class="bar-row" class:active={u.use_case === current.use_case} onclick={() => (selected = u.use_case)}>
              <span class="bar-name"><Icon name={m.icon} size={16} />{m.title}</span>
              <span class="bar"><Meter value={u.score === null ? null : u.score / 100} tone={f.tone} height={10} label="{m.title}: {f.label}" /></span>
              <span class="bar-num">{u.score ?? '–'}</span>
              <span class="bar-fit tone-{f.tone}">{f.short}</span>
            </button>
          </li>
        {/each}
      </ul>
      <p class="faint tiny note">
        Ratings combine the condition above with the processor, memory, storage, graphics, screen and battery.
        Processor and graphics performance are estimated from their model names.
      </p>
    </div>
  </div>
</section>

<style>
  .usefit {
    padding: 28px;
    display: grid;
    gap: 20px;
  }
  header {
    display: grid;
    gap: 4px;
  }
  .picker {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 4px;
    padding: 4px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .picker button {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 10px 4px 8px;
    border-radius: 11px;
    background: transparent;
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .picker button:hover {
    color: var(--text);
    background: color-mix(in srgb, var(--surface) 50%, transparent);
  }
  .picker button.selected {
    background: var(--surface);
    color: var(--accent);
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.06),
      0 3px 10px rgba(0, 0, 0, 0.06);
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: 28px;
    align-items: start;
  }
  .detail {
    display: grid;
    gap: 18px;
  }
  .verdict {
    display: flex;
    align-items: center;
    gap: 20px;
  }
  .ring-num {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  .verdict-text {
    display: grid;
    gap: 2px;
  }
  .verdict-text h3 {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.15;
  }
  .tone-green {
    color: var(--green);
  }
  .tone-amber {
    color: var(--amber);
  }
  .tone-red {
    color: var(--red);
  }
  .tone-grey {
    color: var(--grey);
  }
  .reasons {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .reasons li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .dot {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    margin-top: 1px;
  }
  .dot.good {
    color: var(--green);
    background: var(--green-bg);
  }
  .dot.fair {
    color: var(--amber);
    background: var(--amber-bg);
  }
  .dot.bad {
    color: var(--red);
    background: var(--red-bg);
  }
  .reason-text {
    min-width: 0;
  }
  .unknowns {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--surface-2);
  }
  .chart {
    display: grid;
    gap: 10px;
    padding: 18px;
    border-radius: 16px;
    background: var(--surface-2);
  }
  .chart ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .bar-row {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr) 2.2em 4.4em;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: 10px;
    background: transparent;
    text-align: left;
    font-weight: 500;
    font-size: 13px;
  }
  .bar-row:hover {
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }
  .bar-row.active {
    background: var(--surface);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  }
  .bar-name {
    display: flex;
    align-items: center;
    gap: 7px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--text-2);
  }
  .bar-num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .bar-fit {
    font-size: 12px;
    font-weight: 600;
  }
  .note {
    margin-top: 4px;
  }
  @media (max-width: 860px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 620px) {
    .usefit {
      padding: 20px 16px;
    }
    .picker {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
    .verdict {
      gap: 14px;
    }
    .bar-row {
      grid-template-columns: minmax(0, 1fr) 2.2em 4.4em;
    }
    .bar {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
