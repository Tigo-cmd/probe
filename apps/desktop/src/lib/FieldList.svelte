<script>
  // Label/value rows. Each row is {label, field, format?, mono?}: missing
  // values say "Not reported" with the reason, never a default.
  import ProvenanceTag from './ProvenanceTag.svelte';
  import { field } from './present.js';
  let { rows } = $props();
</script>

<dl>
  {#each rows as row (row.label)}
    {@const f = field(row.field, row.format)}
    <dt>{row.label}</dt>
    <dd title={f.source}>
      {#if f.missing}
        <span class="faint">Not reported{f.note ? ` (${f.note})` : ''}</span>
      {:else}
        <span class:mono={row.mono}>{f.text}</span>
        <ProvenanceTag value={f.provenance} />
      {/if}
    </dd>
  {/each}
</dl>

<style>
  dl {
    display: grid;
    grid-template-columns: minmax(110px, max-content) 1fr;
    gap: 6px 18px;
    margin: 0;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
