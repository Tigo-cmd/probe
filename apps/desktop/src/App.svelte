<script>
  import Home from './lib/Home.svelte';
  import Scanning from './lib/Scanning.svelte';
  import Report from './lib/Report.svelte';
  import { deleteStored, demoMode, listHistory, openSavedScan, openStored, runScan } from './lib/api.js';

  /** @type {'home' | 'scanning' | 'report'} */
  let view = $state('home');
  let opened = $state(null);
  let origin = $state('');
  let error = $state('');
  let history = $state(null);

  async function refreshHistory() {
    try {
      history = await listHistory();
    } catch (e) {
      history = { scans: [], counts: { local: 0, pending: 0, synced: 0, failed: 0 }, location: '', error: String(e) };
    }
  }
  refreshHistory();

  function show(result, from) {
    opened = result;
    origin = from;
    view = 'report';
  }

  async function scan() {
    error = '';
    view = 'scanning';
    try {
      const result = await runScan();
      show(result, demoMode ? 'Design preview: sample data, not a real machine' : 'Live scan of this laptop');
    } catch (e) {
      error = `The scan could not run: ${e}`;
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
  <Scanning />
{:else if view === 'report' && opened}
  <Report {opened} {origin} onback={back} />
{:else}
  <Home
    onscan={scan}
    onopen={open}
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
