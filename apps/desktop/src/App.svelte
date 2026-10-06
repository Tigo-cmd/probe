<script>
  import Home from './lib/Home.svelte';
  import Scanning from './lib/Scanning.svelte';
  import Report from './lib/Report.svelte';
  import { demoMode, openSavedScan, runScan } from './lib/api.js';

  /** @type {'home' | 'scanning' | 'report'} */
  let view = $state('home');
  let graded = $state(null);
  let origin = $state('');
  let error = $state('');

  async function scan() {
    error = '';
    view = 'scanning';
    try {
      graded = await runScan();
      origin = demoMode ? 'Design preview: sample data, not a real machine' : 'Live scan of this laptop';
      view = 'report';
    } catch (e) {
      error = `The scan could not run: ${e}`;
      view = 'home';
    }
  }

  async function open() {
    error = '';
    try {
      const r = await openSavedScan();
      if (!r) return;
      graded = r.graded;
      origin = `Saved scan ${r.path}, re-graded by this version`;
      view = 'report';
    } catch (e) {
      error = String(e);
    }
  }

  function back() {
    view = 'home';
    graded = null;
  }
</script>

{#if demoMode}
  <div class="demo">Design preview with sample data. Not a scan of this machine.</div>
{/if}

{#if view === 'scanning'}
  <Scanning />
{:else if view === 'report' && graded}
  <Report {graded} {origin} onback={back} />
{:else}
  <Home onscan={scan} onopen={open} {error} />
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
