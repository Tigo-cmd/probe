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
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0 6px;
    border-radius: 5px;
    border: 1px solid var(--separator);
    color: var(--text-3);
    white-space: nowrap;
    line-height: 17px;
    vertical-align: 1px;
  }
  .measured {
    color: var(--text-2);
  }
  .claimed {
    border-style: dashed;
  }
  .inferred {
    border-style: dotted;
  }
</style>
