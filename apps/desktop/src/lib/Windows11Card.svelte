<script>
  // Can this computer run Windows 11? One line of verdict, then each of
  // Microsoft's requirements with what was found.
  import Icon from './Icon.svelte';

  let { windows11 } = $props();

  const STATES = {
    ready: { label: 'Ready', tone: 'green' },
    fixable: { label: 'Fixable', tone: 'amber' },
    not_supported: { label: 'Not supported', tone: 'red' },
    unknown: { label: 'Not known', tone: 'grey' },
  };
  const state = $derived(STATES[windows11.readiness] ?? STATES.unknown);
</script>

{#if windows11 && windows11.readiness !== 'not_applicable'}
  <section class="card w11" aria-labelledby="w11-title">
    <div class="head">
      <div>
        <h2 id="w11-title" class="section-title">Windows 11</h2>
        <p class="muted">{windows11.summary}</p>
      </div>
      <span class="pill {state.tone}">{state.label}</span>
    </div>
    <ul class="reqs">
      {#each windows11.requirements as r (r.label)}
        <li class:ok={r.met === true} class:bad={r.met === false}>
          <span class="mark">
            {#if r.met === true}<Icon name="check" size={14} stroke={2.6} />
            {:else if r.met === false}<Icon name="cross" size={14} stroke={2.6} />
            {:else}?{/if}
          </span>
          <span><b>{r.label}</b><span class="faint small">{r.detail}</span></span>
        </li>
      {/each}
    </ul>
    {#if windows11.fixes.length}
      <div class="fixes">
        <p class="small"><b>{windows11.readiness === 'fixable' ? 'How to fix it' : 'Worth doing'}</b></p>
        <ul class="small">
          {#each windows11.fixes as f (f)}<li>{f}</li>{/each}
        </ul>
      </div>
    {/if}
    <p class="faint small">Processor support is worked out from its model name. Graphics and display requirements are met by nearly every computer that meets these.</p>
  </section>
{/if}

<style>
  .w11 {
    padding: 24px 28px;
    display: grid;
    gap: 16px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: flex-start;
  }
  .head p {
    margin-top: 4px;
  }
  .pill {
    padding: 4px 12px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .pill.green {
    background: var(--green-bg);
    color: var(--green);
  }
  .pill.amber {
    background: var(--amber-bg);
    color: var(--amber);
  }
  .pill.red {
    background: var(--red-bg);
    color: var(--red);
  }
  .pill.grey {
    background: var(--surface-3);
    color: var(--text-3);
  }
  .reqs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
  }
  .reqs li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--surface-2);
  }
  .reqs li > span:last-child {
    display: grid;
  }
  .mark {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex: none;
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-3);
    color: var(--text-3);
  }
  .ok .mark {
    background: var(--green-bg);
    color: var(--green);
  }
  .bad .mark {
    background: var(--red-bg);
    color: var(--red);
  }
  .fixes {
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--amber-bg);
  }
  .fixes ul {
    margin: 6px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 4px;
  }
</style>
