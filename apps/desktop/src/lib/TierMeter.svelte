<script>
  // Five segments for a performance tier (1 basic … 5 high-end). An unrated
  // tier shows five empty segments.
  import { tierLabel } from './present.js';
  let { tier = null, tone = 'blue' } = $props();
</script>

<div class="tiers {tone}" role="img" aria-label="Performance: {tierLabel(tier)}">
  {#each [1, 2, 3, 4, 5] as n (n)}
    <span class:on={tier !== null && tier >= n}></span>
  {/each}
</div>

<style>
  .tiers {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
  }
  span {
    height: 8px;
    border-radius: 999px;
    background: var(--track);
    transition: background 400ms;
  }
  .blue .on {
    background: var(--blue-fill);
  }
  .indigo .on {
    background: var(--indigo-fill);
  }
  .teal .on {
    background: var(--teal-fill);
  }
</style>
