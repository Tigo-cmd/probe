<script>
  import { provenance } from './present.js';
  let { value } = $props();
  const p = $derived(provenance(value));
</script>

{#if value}
  <span class="tag {value}" title={p.hint}>{p.label}</span>
{/if}

<style>
  .tag {
    display: inline-block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--text-3);
    white-space: nowrap;
    line-height: 18px;
  }
  .measured {
    border-style: solid;
    color: var(--text-2);
  }
  .claimed {
    border-style: dashed;
  }
  .inferred {
    border-style: dotted;
  }
</style>
