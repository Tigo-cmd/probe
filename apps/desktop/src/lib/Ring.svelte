<script>
  // A ring chart, after Apple's activity rings: a track and an arc that
  // sweeps in on first paint. A missing value draws an empty track, never a
  // full ring. The centre shows whatever the caller passes as children.
  import { ring } from './present.js';

  let { value = null, size = 120, stroke = 12, tone = 'grey', label = '', children } = $props();

  const r = $derived((size - stroke) / 2);
  const g = $derived(ring(value, r));
  let drawn = $state(false);
  $effect(() => {
    const id = requestAnimationFrame(() => (drawn = true));
    return () => cancelAnimationFrame(id);
  });
</script>

<div class="ring {tone}" style="width:{size}px;height:{size}px" role="img" aria-label={label}>
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle class="track" cx={size / 2} cy={size / 2} {r} stroke-width={stroke} />
    {#if value !== null && value !== undefined}
      <circle
        class="arc"
        cx={size / 2}
        cy={size / 2}
        {r}
        stroke-width={stroke}
        stroke-dasharray={g.circumference}
        stroke-dashoffset={drawn ? g.offset : g.circumference}
        transform="rotate(-90 {size / 2} {size / 2})"
      />
    {/if}
  </svg>
  <div class="centre">{@render children?.()}</div>
</div>

<style>
  .ring {
    position: relative;
    flex: none;
  }
  svg {
    display: block;
  }
  circle {
    fill: none;
  }
  .track {
    stroke: var(--track);
  }
  .arc {
    stroke-linecap: round;
    transition: stroke-dashoffset 1100ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .green .arc {
    stroke: var(--green-fill);
  }
  .amber .arc {
    stroke: var(--amber-fill);
  }
  .red .arc {
    stroke: var(--red-fill);
  }
  .grey .arc {
    stroke: var(--grey-fill);
  }
  .blue .arc {
    stroke: var(--blue-fill);
  }
  .centre {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    text-align: center;
  }
</style>
