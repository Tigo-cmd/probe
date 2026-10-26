<script>
  // The local scan history. Deleting takes two clicks: a scan is evidence,
  // and the webview's confirm() is not available on every platform.
  import VerdictBadge from './VerdictBadge.svelte';
  import { historyLine, ORIGINS, osName, scannedAt, SYNC_STATES } from './present.js';

  let { history, onopen, ondelete } = $props();
  let confirming = $state(null);
</script>

<section class="card history" aria-labelledby="history-title">
  <div class="head">
    <div>
      <h2 id="history-title" class="section-title">Your scans</h2>
      <p class="faint small">{historyLine(history.counts)}</p>
    </div>
  </div>

  {#if history.error}
    <p class="error small" role="alert">{history.error}. Scans still run; they are just not kept.</p>
  {:else if history.scans.length === 0}
    <p class="muted small">Scans you run or open are kept here, on this computer, so you can compare a computer with itself later.</p>
  {:else}
    <ul>
      {#each history.scans as s (s.id)}
        <li>
          <button class="row" onclick={() => onopen(s.id)}>
            <span class="when">
              <span>{scannedAt(s.started_at)}</span>
              <span class="faint small">{ORIGINS[s.origin] ?? s.origin} · {osName(s.os)}</span>
            </span>
            <span class="what">
              <span class="machine">{s.machine ?? 'Unidentified machine'}</span>
              <span class="faint small">
                {#if s.serial}<span class="mono">{s.serial}</span>{:else}serial not reported{/if}
                {#if s.label} · <i>{s.label}</i>{/if}
              </span>
            </span>
            <span class="verdict">
              <VerdictBadge value={s.headline} />
              <span class="faint tiny">{SYNC_STATES[s.sync] ?? s.sync}</span>
            </span>
          </button>
          {#if confirming === s.id}
            <span class="confirm">
              <button class="danger" onclick={() => { confirming = null; ondelete(s.id); }}>Delete</button>
              <button onclick={() => (confirming = null)}>Keep</button>
            </span>
          {:else}
            <button class="ghost" aria-label="Delete scan from {scannedAt(s.started_at)}" onclick={() => (confirming = s.id)}>Delete…</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <p class="faint tiny mono where" title="Back up this file to keep your history">{history.location}</p>
</section>

<style>
  .history {
    padding: 24px 26px;
    display: grid;
    gap: 14px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .small {
    font-size: 13px;
  }
  .tiny {
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    border-top: 1px solid var(--separator);
    padding: 4px 0;
  }
  li:first-child {
    border-top: none;
  }
  .row {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(0, 2fr) auto;
    gap: 16px;
    align-items: center;
    text-align: left;
    border-radius: 12px;
    background: transparent;
    padding: 10px 12px;
    font-weight: 400;
    min-width: 0;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .when,
  .what,
  .verdict {
    display: grid;
    gap: 1px;
    min-width: 0;
  }
  .verdict {
    justify-items: end;
  }
  .machine {
    font-weight: 600;
  }
  .what span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ghost {
    background: transparent;
    color: var(--text-3);
    font-weight: 500;
    font-size: 13px;
    padding: 4px 8px;
  }
  .confirm {
    display: flex;
    gap: 6px;
  }
  .confirm button {
    font-size: 13px;
    padding: 4px 10px;
  }
  .danger {
    color: #fff;
    background: var(--red-fill);
  }
  .error {
    color: var(--red);
    background: var(--red-bg);
    padding: 10px 14px;
    border-radius: 12px;
  }
  .where {
    overflow-wrap: anywhere;
  }
  @media (max-width: 640px) {
    .row {
      grid-template-columns: 1fr auto;
    }
    .when {
      grid-column: 1 / -1;
    }
    li {
      flex-wrap: wrap;
    }
  }
</style>
