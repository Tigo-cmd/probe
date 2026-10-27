<script>
  // Selling: make a report with a QR code for buyers. Buying: paste the link
  // from a seller's report and compare it with this computer.
  import Icon from './Icon.svelte';
  import { checkSellerLink, saveSellerReportAs } from './api.js';

  let { scan, recordId = null, onwipeguide } = $props();

  let status = $state('');
  let failure = $state('');
  let link = $state('');
  let checking = $state(false);
  let check = $state(null);

  async function makeReport() {
    status = '';
    failure = '';
    try {
      const path = await saveSellerReportAs(scan, recordId);
      if (path) status = `Saved to ${path} and opened in your browser. Print it and choose Save as PDF to share it as a PDF.`;
    } catch (e) {
      failure = String(e);
    }
  }

  async function runCheck() {
    failure = '';
    check = null;
    checking = true;
    try {
      check = await checkSellerLink(link.trim(), scan);
    } catch (e) {
      failure = String(e);
    } finally {
      checking = false;
    }
  }

  const VERDICT = {
    same: { tone: 'green', title: 'This is the computer in the report', icon: 'check' },
    different: { tone: 'red', title: 'This is a different computer from the report', icon: 'cross' },
    unknown: { tone: 'amber', title: 'Not enough identifiers to tell whether it is the same computer', icon: 'question' },
  };
  const date = (t) => (t ? new Date(t * 1000).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }) : 'an unknown date');
</script>

<section class="card seller" aria-labelledby="seller-title">
  <h2 id="seller-title" class="section-title">Selling or buying?</h2>
  <div class="cols">
    <div class="col">
      <h3>Selling it</h3>
      <p class="muted small">Make a seller's report with a QR code. Buyers scan it to see the results on their phone, then check them with probe.</p>
      <div class="row">
        <button class="primary" onclick={makeReport}><Icon name="doc" size={16} /> Seller's report…</button>
        {#if onwipeguide}<button class="plain" onclick={onwipeguide}>How to wipe it safely</button>{/if}
      </div>
    </div>
    <div class="col">
      <h3>Buying it</h3>
      <p class="muted small">Paste the link from the seller's report or its QR code to compare it with this computer.</p>
      <form class="row" onsubmit={(e) => (e.preventDefault(), runCheck())}>
        <input type="url" placeholder="https://probe-pc.github.io/check#…" bind:value={link} aria-label="Seller's report link" />
        <button type="submit" disabled={!link.trim() || checking}>{checking ? 'Checking…' : 'Check'}</button>
      </form>
    </div>
  </div>

  {#if status}<p class="status small" role="status">{status}</p>{/if}
  {#if failure}<p class="status error small" role="alert">{failure}</p>{/if}

  {#if check}
    {@const v = VERDICT[check.identity] ?? VERDICT.unknown}
    <div class="result {v.tone}">
      <div class="result-head">
        <span class="mark"><Icon name={v.icon} size={16} stroke={2.6} /></span>
        <div>
          <b>{v.title}</b>
          <p class="faint small">
            Report for {check.seller.m}, scanned {date(check.seller.t)}. {check.matched} of {check.compared} identifiers match.
          </p>
        </div>
      </div>
      {#if check.differences.length}
        <ul class="small">
          {#each check.differences as d (d)}<li>{d}</li>{/each}
        </ul>
      {:else if check.identity === 'same'}
        <p class="small">Nothing has got worse since the seller's scan.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .seller {
    padding: 24px 28px;
    display: grid;
    gap: 16px;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }
  .col {
    display: grid;
    gap: 8px;
    align-content: start;
  }
  h3 {
    font-size: 15px;
    font-weight: 650;
  }
  .row button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  input {
    flex: 1;
    min-width: 0;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--separator);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 14px;
  }
  .status {
    margin: 0;
  }
  .error {
    color: var(--red);
  }
  .result {
    border-radius: 14px;
    padding: 14px 16px;
    display: grid;
    gap: 8px;
  }
  .result.green {
    background: var(--green-bg);
  }
  .result.red {
    background: var(--red-bg);
  }
  .result.amber {
    background: var(--amber-bg);
  }
  .result-head {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .mark {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex: none;
    background: var(--surface);
  }
  .result ul {
    margin: 0;
    padding-left: 20px;
    display: grid;
    gap: 4px;
  }
  @media (max-width: 760px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
