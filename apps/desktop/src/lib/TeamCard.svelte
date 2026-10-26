<script>
  // Team sharing: connect this computer to a probe-sync server. Nothing is
  // shared by connecting; each scan is shared from its report.
  import Icon from './Icon.svelte';
  import { syncNow, teamConnect, teamDisconnect } from './api.js';
  import { syncReport } from './present.js';

  let { team, counts = null, onchange, onsynced } = $props();

  let server = $state('');
  let token = $state('');
  let busy = $state(false);
  let message = $state('');
  let failure = $state('');
  let editing = $state(false);

  const queued = $derived(counts ? counts.pending + counts.failed : 0);

  async function run(fn) {
    busy = true;
    message = '';
    failure = '';
    try {
      await fn();
    } catch (e) {
      failure = String(e?.message ?? e);
    } finally {
      busy = false;
    }
  }

  const connect = () =>
    run(async () => {
      onchange(await teamConnect(server, token));
      token = '';
      editing = false;
      message = 'Connected. Share a scan from its report.';
    });
  const disconnect = () =>
    run(async () => {
      onchange(await teamDisconnect());
      message = 'Disconnected. Scans already shared stay on the server.';
    });
  const sync = () =>
    run(async () => {
      message = syncReport(await syncNow());
      onsynced?.();
    });
</script>

<section class="card team" aria-labelledby="team-title">
  <div class="head">
    <span class="badge"><Icon name="folder" size={20} /></span>
    <div>
      <h2 id="team-title" class="section-title">Team sharing</h2>
      <p class="faint small">
        Share chosen scans with your shop or team, and see when a computer has been scanned before.
      </p>
    </div>
  </div>

  {#if team?.configured && !editing}
    <div class="connected">
      <span class="dot" aria-hidden="true"></span>
      <div class="who">
        <p>Connected{team.device ? ` as ${team.device}` : ''}</p>
        <p class="faint small mono">{team.server}</p>
      </div>
      <div class="actions">
        <button onclick={sync} disabled={busy}>{queued ? `Share ${queued} queued` : 'Sync now'}</button>
        <button class="plain" onclick={() => (editing = true)} disabled={busy}>Change</button>
        <button class="plain danger" onclick={disconnect} disabled={busy}>Disconnect</button>
      </div>
    </div>
  {:else}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        connect();
      }}
    >
      <label>
        <span class="small">Server address</span>
        <input type="url" bind:value={server} placeholder="https://sync.your-shop.com" required autocomplete="url" />
      </label>
      <label>
        <span class="small">Access token</span>
        <input type="password" bind:value={token} placeholder="probe_…" required autocomplete="off" spellcheck="false" />
      </label>
      <div class="form-actions">
        <button class="primary" type="submit" disabled={busy || !server || !token}>{busy ? 'Checking…' : 'Connect'}</button>
        {#if editing}<button type="button" class="plain" onclick={() => (editing = false)}>Cancel</button>{/if}
      </div>
      <p class="faint tiny">
        Whoever runs your team's server gives you the token (<span class="mono">probe-sync token add NAME</span>).
        Scans include serial numbers, so the server must use https. Connecting shares nothing by itself.
      </p>
    </form>
  {/if}

  {#if message}<p class="msg small" role="status">{message}</p>{/if}
  {#if failure}<p class="msg error small" role="alert">{failure}</p>{/if}
</section>

<style>
  .team {
    padding: 24px 26px;
    display: grid;
    gap: 16px;
  }
  .head {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .badge {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: #fff;
    background: var(--indigo-fill);
  }
  .connected {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 14px;
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--green-fill);
    box-shadow: 0 0 0 4px var(--green-bg);
  }
  .who {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .who p:first-child {
    font-weight: 600;
  }
  .actions {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .danger {
    color: var(--red);
  }
  .form {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 14px;
  }
  label {
    display: grid;
    gap: 4px;
    color: var(--text-2);
  }
  input {
    font: inherit;
    font-size: 14px;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--separator);
    background: var(--surface);
    color: var(--text);
    min-width: 0;
  }
  .form-actions {
    grid-column: 1 / -1;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .form .tiny {
    grid-column: 1 / -1;
  }
  .msg {
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--surface-2);
  }
  .msg.error {
    color: var(--red);
    background: var(--red-bg);
  }
  @media (max-width: 620px) {
    .form {
      grid-template-columns: 1fr;
    }
    .team {
      padding: 20px 16px;
    }
  }
</style>
