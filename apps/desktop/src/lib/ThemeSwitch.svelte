<script>
  // System / Light / Dark, as a small segmented control.
  import Icon from './Icon.svelte';
  import { getTheme, setTheme, THEMES } from './theme.js';

  let theme = $state(getTheme());
  const LABELS = { system: 'Match system', light: 'Light', dark: 'Dark' };
  const ICONS = { system: 'auto', light: 'sun', dark: 'moon' };

  function choose(t) {
    theme = t;
    setTheme(t);
  }
</script>

<div class="switch" role="radiogroup" aria-label="Appearance">
  {#each THEMES as t (t)}
    <button
      role="radio"
      aria-checked={theme === t}
      class:on={theme === t}
      title={LABELS[t]}
      aria-label={LABELS[t]}
      onclick={() => choose(t)}
    >
      <Icon name={ICONS[t]} size={16} />
    </button>
  {/each}
</div>

<style>
  /* A fixed-size pill so it never stretches with its container. */
  .switch {
    display: inline-grid;
    grid-auto-flow: column;
    grid-auto-columns: 28px;
    align-items: center;
    justify-items: center;
    gap: 2px;
    box-sizing: border-box;
    height: 32px;
    padding: 3px;
    border-radius: 16px;
    background: var(--surface-3);
    align-self: center;
    flex: none;
    line-height: 0;
  }
  .switch button {
    box-sizing: border-box;
    width: 28px;
    height: 26px;
    min-height: 0;
    margin: 0;
    padding: 0;
    border: 0;
    display: grid;
    place-items: center;
    border-radius: 13px;
    background: transparent;
    color: var(--text-3);
    line-height: 0;
    appearance: none;
    -webkit-appearance: none;
  }
  .switch button:hover {
    color: var(--text);
    background: transparent;
  }
  .switch button.on {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }
</style>
