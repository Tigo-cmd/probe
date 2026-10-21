<script>
  // A horizontal bar. `value` is 0–1; a missing value draws only the track.
  // An optional `mark` (0–1) draws a reference line, such as design capacity.
  let { value = null, tone = 'blue', height = 8, mark = null, label = '' } = $props();
  const pct = $derived(typeof value === 'number' ? Math.max(0, Math.min(1, value)) * 100 : 0);
  let drawn = $state(false);
  $effect(() => {
    const id = requestAnimationFrame(() => (drawn = true));
    return () => cancelAnimationFrame(id);
  });
</script>

<div class="meter {tone}" style="height:{height}px" role="img" aria-label={label}>
  {#if typeof value === 'number'}
    <span class="fill" style="width:{drawn ? pct : 0}%"></span>
  {/if}
  {#if typeof mark === 'number'}
    <span class="mark" style="left:{Math.max(0, Math.min(1, mark)) * 100}%"></span>
  {/if}
</div>

<style>
  .meter {
    position: relative;
    width: 100%;
    background: var(--track);
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 999px;
    transition: width 900ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .mark {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--text-3);
    opacity: 0.6;
  }
  .green .fill {
    background: var(--green-fill);
  }
  .amber .fill {
    background: var(--amber-fill);
  }
  .red .fill {
    background: var(--red-fill);
  }
  .grey .fill {
    background: var(--grey-fill);
  }
  .blue .fill {
    background: var(--blue-fill);
  }
  .indigo .fill {
    background: var(--indigo-fill);
  }
  .teal .fill {
    background: var(--teal-fill);
  }
</style>
