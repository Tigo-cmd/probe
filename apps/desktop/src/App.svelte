<script>
  import Home from './lib/Home.svelte';
  import Scanning from './lib/Scanning.svelte';
  import Report from './lib/Report.svelte';
  import {
    deleteStored,
    demoMode,
    listHistory,
    openSavedScan,
    openStored,
    privilegeStatus,
    runScan,
  } from './lib/api.js';
  import { scanFailure } from './lib/present.js';

  /** @type {'home' | 'scanning' | 'report'} */
  let view = $state('home');
  let opened = $state(null);
  let origin = $state('');
  let error = $state('');
  let history = $state(null);
  let privilege = $state(null);
  /** True while the report shows a live scan of this machine. */
  let live = $state(false);

  async function refreshHistory() {
    try {
      history = await listHistory();
    } catch (e) {
      history = { scans: [], counts: { local: 0, pending: 0, synced: 0, failed: 0 }, location: '', error: String(e) };
    }
  }
  refreshHistory();
  privilegeStatus()
    .then((p) => (privilege = p))
    .catch(() => (privilege = null));

  function show(result, from, isLive = false) {
    opened = result;
    origin = from;
    live = isLive;
    view = 'report';
  }

  /** @param {{full: boolean}} options full: ask for administrator rights first */
  async function scan({ full }) {
    error = '';
    view = 'scanning';
    try {
      const result = await runScan({ full });
      const from = demoMode
        ? 'Design preview: sample data, not a real machine'
        : result.scan.elevated
          ? 'Live scan of this laptop, full access'
          : 'Live scan of this laptop, without administrator rights';
      show(result, from, true);
    } catch (e) {
      error = scanFailure(e).text;
      view = 'home';
    }
  }

  async function open() {
    error = '';
    try {
      const r = await openSavedScan();
      if (r) show(r.opened, `Saved scan ${r.path}, re-graded by this version`);
    } catch (e) {
      error = String(e);
    }
  }

  async function openFromHistory(id) {
    error = '';
    try {
      const r = await openStored(id);
      show(r, 'From scan history, re-graded by this version');
    } catch (e) {
      error = String(e);
    }
  }

  async function removeFromHistory(id) {
    error = '';
    try {
      await deleteStored(id);
    } catch (e) {
      error = String(e);
    }
    await refreshHistory();
  }

  function back() {
    view = 'home';
    opened = null;
    refreshHistory();
  }
</script>

{#if demoMode}
  <div class="demo">Design preview with sample data. Not a scan of this machine.</div>
{/if}

{#if view === 'scanning'}
  <Scanning asking={privilege?.can_elevate ? privilege.method : ''} />
{:else if view === 'report' && opened}
  <Report
    {opened}
    {origin}
    onback={back}
    onrescan={live && privilege?.can_elevate && !opened.scan.elevated ? () => scan({ full: true }) : null}
  />
{:else}
  <Home
    onscan={() => scan({ full: true })}
    onscanlimited={() => scan({ full: false })}
    onopen={open}
    {privilege}
    {history}
    onopenstored={openFromHistory}
    ondeletestored={removeFromHistory}
    {error}
  />
{/if}

<style>
  .demo {
    background: var(--accent);
    color: var(--accent-text);
    text-align: center;
    font-size: 13px;
    padding: 4px 12px;
  }
</style>
