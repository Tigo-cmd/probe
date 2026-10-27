<script>
  // A small card in the corner when a newer probe is out. Installing is
  // always the person's choice, and never offered in the middle of a scan.
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { findUpdate, RECHECK_MS, skip, wasSkipped } from './updates.js';

  let { busy = false } = $props();

  let update = $state(null);
  /** 'offer' | 'installing' | 'failed' */
  let phase = $state('offer');
  let progress = $state(null);
  let failure = $state('');

  async function look() {
    if (phase === 'installing') return;
    const found = await findUpdate();
    if (found && !wasSkipped(found.version)) {
      update = found;
      phase = 'offer';
    }
  }

  onMount(() => {
    const first = setTimeout(look, 3000);
    const again = setInterval(look, RECHECK_MS);
    return () => {
      clearTimeout(first);
      clearInterval(again);
    };
  });

  async function install() {
    phase = 'installing';
    progress = 0;
    try {
      await update.install((p) => (progress = p));
    } catch (e) {
      failure = String(e?.message ?? e);
      phase = 'failed';
    }
  }

  function later() {
    skip(update.version);
    update = null;
  }
</script>

{#if update}
  <aside class="update card" role="status" aria-live="polite">
    <div class="head">
      <span class="badge"><Icon name="sparkle" size={18} /></span>
      <div>
        <h2>probe {update.version} is available</h2>
        <p class="faint small">You have {update.current}</p>
      </div>
    </div>

    {#if phase === 'offer'}
      {#if update.notes.length}
        <ul class="notes small">
          {#each update.notes as n (n)}<li>{n}</li>{/each}
        </ul>
      {/if}
      <div class="actions">
        <button class="plain" onclick={later}>Later</button>
        <button class="primary" onclick={install} disabled={busy}>Update and restart</button>
      </div>
      {#if busy}<p class="faint small">Available once the scan finishes.</p>{/if}
    {:else if phase === 'installing'}
      <div class="bar"><i style="width: {progress === null ? 100 : Math.round(progress * 100)}%" class:busy={progress === null}></i></div>
      <p class="muted small">
        {progress !== null && progress < 1 ? `Downloading… ${Math.round(progress * 100)}%` : 'Installing. probe will restart in a moment.'}
      </p>
    {:else}
      <p class="error small">The update could not be installed: {failure}</p>
      <div class="actions">
        <button class="plain" onclick={() => (update = null)}>Close</button>
        <button class="primary" onclick={install}>Try again</button>
      </div>
      <p class="faint small">You can also download it from probe-pc.github.io.</p>
    {/if}
  </aside>
{/if}

<style>
  .update {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 100;
    width: min(360px, calc(100vw - 40px));
    padding: 18px;
    display: grid;
    gap: 12px;
    box-shadow: var(--shadow-lift);
    animation: rise 320ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(16px);
    }
  }
  .head {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .badge {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--blue-fill);
    color: #fff;
    flex: none;
  }
  h2 {
    font-size: 15px;
    font-weight: 650;
  }
  .notes {
    margin: 0;
    padding-left: 18px;
    color: var(--text-2);
    display: grid;
    gap: 3px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .bar {
    height: 6px;
    border-radius: 99px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--blue-fill);
    border-radius: 99px;
    transition: width 200ms;
  }
  .bar i.busy {
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.45;
    }
  }
  .error {
    color: var(--red);
  }
  @media (prefers-reduced-motion: reduce) {
    .update,
    .bar i.busy {
      animation: none;
    }
  }
</style>
