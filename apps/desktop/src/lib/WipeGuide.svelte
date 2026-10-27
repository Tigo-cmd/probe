<script>
  // Step by step: what a seller should do before handing a computer over.
  import Icon from './Icon.svelte';
  import ThemeSwitch from './ThemeSwitch.svelte';
  import { openLink } from './api.js';
  import { WIPE_GUIDES, wipeOs } from './wipe.js';

  let { os = null, onback } = $props();
  let chosen = $state(wipeOs(os));
  const guide = $derived(WIPE_GUIDES[chosen]);
  let done = $state({});
  const count = $derived(guide.steps.filter((s) => done[`${chosen}:${s.title}`]).length);
</script>

<div class="toolbar">
  <div class="toolbar-inner">
    <button class="back" onclick={onback}><Icon name="back" size={18} /> Back</button>
    <div class="spacer"></div>
    <ThemeSwitch />
  </div>
</div>

<main class="guide">
  <header>
    <p class="eyebrow">Selling a computer</p>
    <h1>Wipe it safely before you hand it over</h1>
    <p class="muted lede">
      Make your seller's report first: it describes the hardware, which wiping does not change. Then work through these
      steps so the buyer gets a clean computer and none of your accounts or files.
    </p>
  </header>

  <div class="tabs" role="tablist" aria-label="System">
    {#each Object.entries(WIPE_GUIDES) as [id, g] (id)}
      <button role="tab" aria-selected={chosen === id} class:on={chosen === id} onclick={() => (chosen = id)}>{g.label}</button>
    {/each}
  </div>

  <p class="faint small">{count} of {guide.steps.length} done</p>
  <ol class="steps">
    {#each guide.steps as s, i (s.title)}
      {@const key = `${chosen}:${s.title}`}
      <li class="card step" class:done={done[key]}>
        <label>
          <input type="checkbox" bind:checked={done[key]} />
          <span class="num">{i + 1}</span>
        </label>
        <div>
          <h2>{s.title}</h2>
          <p class="muted">{s.how}</p>
          {#if s.warn}<p class="warn small"><Icon name="caution" size={14} /> {s.warn}</p>{/if}
          {#if s.link}<button class="plain small" onclick={() => openLink(s.link.url)}>{s.link.label} ↗</button>{/if}
        </div>
      </li>
    {/each}
  </ol>
</main>

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 5;
    background: color-mix(in srgb, var(--bg) 72%, transparent);
    backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--separator);
  }
  .toolbar-inner {
    max-width: 820px;
    margin: 0 auto;
    padding: 10px 24px;
    display: flex;
    align-items: center;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    color: var(--accent);
    padding-left: 4px;
  }
  .spacer {
    flex: 1;
  }
  .guide {
    max-width: 820px;
    margin: 0 auto;
    padding: 28px 24px 56px;
    display: grid;
    gap: 18px;
  }
  h1 {
    font-size: 32px;
    font-weight: 700;
    letter-spacing: -0.025em;
    margin-top: 4px;
  }
  .lede {
    margin-top: 8px;
    max-width: 64ch;
  }
  .tabs {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border-radius: 12px;
    background: var(--surface-3);
    justify-self: start;
  }
  .tabs button {
    background: none;
    border-radius: 9px;
    padding: 6px 16px;
  }
  .tabs button.on {
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 12px;
  }
  .step {
    display: flex;
    gap: 16px;
    padding: 18px 20px;
    align-items: flex-start;
  }
  .step.done {
    opacity: 0.6;
  }
  .step label {
    display: grid;
    justify-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .step input {
    width: 18px;
    height: 18px;
    accent-color: var(--green-fill);
  }
  .num {
    font-size: 12px;
    color: var(--text-3);
  }
  .step h2 {
    font-size: 16px;
    font-weight: 650;
    margin-bottom: 4px;
  }
  .warn {
    margin-top: 8px;
    color: var(--amber);
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .plain.small {
    margin-top: 6px;
    padding: 0;
    font-size: 13px;
  }
</style>
