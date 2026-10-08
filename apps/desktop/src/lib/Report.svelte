<script>
  import VerdictBadge from './VerdictBadge.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import FieldList from './FieldList.svelte';
  import {
    axisLabel,
    axisSummary,
    AXES,
    capacityShare,
    describeDivergence,
    driveHealth,
    elevationHint,
    formatBytes,
    formatCapacity,
    formatGiB,
    ORIGINS,
    osName,
    relativeAge,
    scannedAt,
    sortFindings,
    TRANSPORTS,
    verdict,
  } from './present.js';
  import { compareStored, compareWithSaved, labelStored, saveScanAs, saveTextReportAs } from './api.js';

  let { opened, origin, onback, onrescan = null } = $props();
  const scan = $derived(opened.scan);
  const grade = $derived(opened.grade);
  // Writable deriveds: they follow the opened scan and accept local edits.
  let record = $derived(opened.record);
  let label = $derived(opened.record?.label ?? '');
  const head = $derived(verdict(grade.headline));

  let status = $state('');
  let failure = $state('');
  let comparison = $state(null);

  async function act(fn, done) {
    status = '';
    failure = '';
    try {
      const r = await fn();
      if (r) status = done(r);
    } catch (e) {
      failure = String(e);
    }
  }
  const saveScan = () => act(() => saveScanAs(scan), (p) => `Scan saved to ${p}`);
  const saveText = () => act(() => saveTextReportAs(scan), (p) => `Text report saved to ${p}`);
  const compare = () =>
    act(
      () => compareWithSaved(scan),
      (r) => {
        comparison = { title: r.path, comparison: r.comparison };
        return '';
      },
    );
  const compareEarlier = (s) =>
    act(
      () => compareStored(s.id, scan),
      (c) => {
        comparison = { title: "From scan history", comparison: c };
        return '';
      },
    );
  async function saveLabel() {
    if (!record || (record.label ?? '') === label.trim()) return;
    await act(
      () => labelStored(record.id, label.trim()),
      (r) => {
        record = r;
        return 'Note saved';
      },
    );
  }

  const capacity = (c) => formatCapacity(c);
</script>

<div class="toolbar">
  <div class="toolbar-inner">
    <button onclick={onback}>← New scan</button>
    <span class="origin faint" title={origin}>{origin}</span>
    <div class="spacer"></div>
    <button onclick={compare}>Compare with earlier scan…</button>
    <button onclick={saveText}>Save text report…</button>
    <button class="primary" onclick={saveScan}>Save scan…</button>
  </div>
</div>

<main class="report">
  {#if status}<p class="status" role="status">{status}</p>{/if}
  {#if failure}<p class="status error" role="alert">{failure}</p>{/if}

  <section class="headline card {grade.headline}">
    <div class="headline-main">
      <VerdictBadge value={grade.headline} size="lg" />
      <h1>{head.headline}</h1>
      {#if grade.limiting_axis}
        <p class="muted">Limited by {axisLabel(grade.limiting_axis).toLowerCase()}. The overall result is the worst axis, never an average.</p>
      {:else if grade.headline === 'green'}
        <p class="muted">No faults detected in the checks that could run. That lowers risk; it does not clear the machine.</p>
      {/if}
      {#if !grade.complete}
        <p class="muted">Incomplete: at least one axis could not be graded, so this is not a full result.</p>
      {/if}
    </div>
    <dl class="meta">
      <dt>Machine</dt>
      <dd>{[scan.identity.vendor?.value, scan.identity.model?.value].filter(Boolean).join(' ') || 'Not reported'}</dd>
      <dt>Scanned</dt>
      <dd>{scannedAt(scan.started_at)} on {osName(scan.os)}</dd>
      <dt>Versions</dt>
      <dd>probe {scan.tool_version} · grader {grade.grader_version}</dd>
    </dl>
  </section>

  {#if !scan.elevated}
    <p class="banner">
      <b>Partial scan.</b> It ran without administrator rights, so some reads were skipped.
      {#if onrescan}
        <button class="rescan" onclick={onrescan}>Rescan with administrator rights</button>
      {:else}
        {elevationHint(scan.os)}
      {/if}
    </p>
  {/if}

  {#if record}
    <section class="kept">
      <span class="faint small">Kept in scan history · {ORIGINS[record.origin] ?? record.origin} · on this computer only</span>
      <label class="note">
        <span class="faint small">Note</span>
        <input
          type="text"
          bind:value={label}
          placeholder="Listing, seller or asking price"
          maxlength="200"
          onblur={saveLabel}
          onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
        />
      </label>
    </section>
  {:else if opened.history_error}
    <p class="banner">Not kept in scan history: {opened.history_error}</p>
  {/if}

  {#if opened.earlier?.length}
    <section class="card block earlier">
      <div class="block-head">
        <h2>Earlier scans of this machine</h2>
        <span class="faint small">Matched on UUID or serial, which are claims</span>
      </div>
      <p class="muted small">Compare to see whether drives, battery, board or network cards changed in between.</p>
      <ul class="plain">
        {#each opened.earlier as s (s.id)}
          <li class="earlier-row">
            <VerdictBadge value={s.headline} />
            <span>{scannedAt(s.started_at)}</span>
            <span class="faint small">{relativeAge(s.started_at, scan.started_at)} · {ORIGINS[s.origin] ?? s.origin}{s.label ? ` · ${s.label}` : ''}</span>
            <span class="spacer"></span>
            <button onclick={() => compareEarlier(s)}>Compare</button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="axes">
    {#each grade.axes as axis (axis.axis)}
      <a class="axis card" href="#axis-{axis.axis}">
        <div class="axis-top">
          <h2>{axisLabel(axis.axis)}</h2>
          <VerdictBadge value={axis.verdict} />
        </div>
        <p class="faint small">{AXES[axis.axis]?.blurb}</p>
        <p class="summary">{axisSummary(axis)}</p>
      </a>
    {/each}
  </section>

  {#if comparison}
    {@const c = comparison.comparison}
    <section class="card block compare {c.divergences.length ? 'diverged' : ''}">
      <h2>Compared with an earlier scan</h2>
      <p class="faint small mono">{comparison.title}, scanned {scannedAt(c.earlier_started_at)}</p>
      {#if c.divergences.length}
        <p><b>{c.divergences.length} identifier{c.divergences.length === 1 ? '' : 's'} changed.</b> Parts may have been swapped since the earlier scan. Ask the seller why.</p>
        <ul>
          {#each c.divergences as d, i (i)}
            <li class="mono">{describeDivergence(d)}</li>
          {/each}
        </ul>
      {:else}
        <p>No divergence between identifiers present in both scans ({c.matching} matching).</p>
      {/if}
      {#if c.matching === 0}
        <p class="muted">No identifier matched. These may be scans of different machines.</p>
      {/if}
    </section>
  {/if}

  {#each grade.axes as axis (axis.axis)}
    <section class="card block" id="axis-{axis.axis}">
      <div class="block-head">
        <h2>{axisLabel(axis.axis)}</h2>
        <VerdictBadge value={axis.verdict} />
      </div>
      {#if axis.findings.length === 0}
        <p class="faint">No checks produced a result for this axis.</p>
      {/if}
      <ul class="findings">
        {#each sortFindings(axis.findings) as f, i (i)}
          <li>
            <VerdictBadge value={f.verdict} />
            <div class="finding">
              <p>{f.message} <ProvenanceTag value={f.provenance} /></p>
              <p class="faint small basis"><span class="mono">{f.component}</span> · Basis: {f.basis}</p>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/each}

  <section class="card block">
    <h2>This machine</h2>
    <div class="cols">
      <FieldList
        rows={[
          { label: 'Vendor', field: scan.identity.vendor },
          { label: 'Model', field: scan.identity.model },
          { label: 'Version', field: scan.identity.version },
          { label: 'Serial', field: scan.identity.serial, mono: true },
          { label: 'Board serial', field: scan.identity.board_serial, mono: true },
          { label: 'UUID', field: scan.identity.uuid, mono: true },
        ]}
      />
      <FieldList
        rows={[
          { label: 'CPU', field: scan.cpu.brand },
          { label: 'Cores', field: scan.cpu.physical_cores },
          { label: 'Threads', field: scan.cpu.logical_cores },
          { label: 'RAM', field: scan.memory.total_bytes, format: formatGiB },
          { label: 'Firmware', field: scan.identity.bios_version },
          { label: 'Firmware date', field: scan.identity.bios_date },
        ]}
      />
    </div>
  </section>

  {#each scan.batteries as b (b.name)}
    {@const share = capacityShare(b)}
    <section class="card block">
      <div class="block-head">
        <h2>Battery {b.name}</h2>
        {#if share !== null}
          <span class="share">Holds <b>{share}%</b> of its design capacity <ProvenanceTag value="measured" /></span>
        {/if}
      </div>
      <div class="cols">
        <FieldList
          rows={[
            { label: 'Design', field: b.design_capacity, format: capacity },
            { label: 'Full charge', field: b.full_charge_capacity, format: capacity },
            { label: 'Cycles', field: b.cycle_count },
          ]}
        />
        <FieldList
          rows={[
            { label: 'Manufacturer', field: b.manufacturer },
            { label: 'Model', field: b.model },
            { label: 'Serial', field: b.serial, mono: true },
            { label: 'Made', field: b.manufacture_date },
          ]}
        />
      </div>
    </section>
  {/each}

  {#each scan.storage as d (d.name)}
    {@const h = driveHealth(d)}
    <section class="card block">
      <div class="block-head">
        <h2>Drive {d.name}</h2>
        <span class="faint">{TRANSPORTS[d.transport] ?? d.transport}{d.removable ? ' · removable' : ''}</span>
      </div>
      <p class="small {h.read ? 'muted' : 'faint'}">{h.read ? '' : 'Health not read: '}{h.text}</p>
      <div class="cols">
        <FieldList
          rows={[
            { label: 'Model', field: d.model },
            { label: 'Serial', field: d.serial, mono: true },
            { label: 'Firmware', field: d.firmware, mono: true },
          ]}
        />
        <FieldList
          rows={[
            { label: 'Capacity', field: d.capacity_bytes, format: formatBytes },
            { label: 'Spinning disk', field: d.rotational, format: (v) => (v ? 'Yes' : 'No') },
          ]}
        />
      </div>
    </section>
  {/each}

  {#if scan.displays.length || scan.network.length}
    <section class="card block">
      <h2>Displays and network</h2>
      <ul class="plain">
        {#each scan.displays as d, i (i)}
          <li>
            {d.manufacturer_id ?? '?'} {d.name ?? ''}
            {#if d.native_width}<span class="muted">{d.native_width}×{d.native_height}</span>{/if}
            {#if d.manufacture_year}<span class="muted">made {d.manufacture_year}</span>{/if}
            <ProvenanceTag value="claimed" />
          </li>
        {/each}
        {#each scan.network as n (n.mac)}
          <li>{n.name} <span class="mono muted">{n.mac}</span> <ProvenanceTag value="claimed" /></li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="two">
    <div class="card block">
      <h2>Not assessable by software</h2>
      <p class="faint small">Check these yourself. Their absence here is not a pass.</p>
      <ul class="bullets">
        {#each grade.not_assessable as n (n)}<li>{n}</li>{/each}
      </ul>
    </div>
    <div class="card block">
      <h2>Read this</h2>
      <ul class="bullets">
        {#each grade.caveats as c (c)}<li>{c}</li>{/each}
      </ul>
    </div>
  </section>

  {#if scan.probe_notes.length}
    <details class="card block">
      <summary><h2>Probe notes ({scan.probe_notes.length})</h2></summary>
      <p class="faint small">Reads that failed or were skipped. Each one is missing evidence, not a clean result.</p>
      <ul class="plain small">
        {#each scan.probe_notes as n, i (i)}
          <li><span class="mono">{n.component}</span> <span class="muted">{n.message}</span></li>
        {/each}
      </ul>
    </details>
  {/if}
</main>

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 2;
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--border);
  }
  .toolbar-inner {
    max-width: 1040px;
    margin: 0 auto;
    padding: 10px 24px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .origin {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 32ch;
    font-size: 13px;
  }
  .spacer {
    flex: 1;
  }
  .report {
    max-width: 1040px;
    margin: 0 auto;
    padding: 20px 24px 64px;
    display: grid;
    gap: 16px;
  }
  .status {
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--border);
    overflow-wrap: anywhere;
  }
  .status.error {
    color: var(--red);
    background: var(--red-bg);
    border-color: transparent;
  }
  .headline {
    padding: 24px 28px;
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    justify-content: space-between;
    border-left: 6px solid var(--grey);
  }
  .headline.green {
    border-left-color: var(--green);
  }
  .headline.amber {
    border-left-color: var(--amber);
  }
  .headline.red {
    border-left-color: var(--red);
  }
  .headline-main {
    display: grid;
    gap: 8px;
    justify-items: start;
    max-width: 56ch;
  }
  h1 {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .meta {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 0;
    font-size: 14px;
    align-content: start;
  }
  .meta dt {
    color: var(--text-3);
  }
  .meta dd {
    margin: 0;
  }
  .rescan {
    margin-left: 6px;
    padding: 3px 12px;
    font-size: 14px;
  }
  .banner {
    background: var(--amber-bg);
    color: var(--text);
    border: 1px solid color-mix(in srgb, var(--amber) 30%, transparent);
    padding: 10px 14px;
    border-radius: var(--radius-sm);
  }
  .kept {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    max-width: 440px;
  }
  .note input {
    flex: 1;
    font: inherit;
    font-size: 14px;
    padding: 5px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    min-width: 0;
  }
  .note input:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  .earlier-row {
    align-items: center;
  }
  .axes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
  }
  .axis {
    padding: 16px 18px;
    display: grid;
    gap: 4px;
    color: inherit;
    text-decoration: none;
  }
  .axis:hover {
    border-color: var(--text-3);
  }
  .axis-top,
  .block-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 16px;
    font-weight: 650;
  }
  .summary {
    font-weight: 550;
    margin-top: 6px;
  }
  .small {
    font-size: 13px;
  }
  .block {
    padding: 18px 22px;
    display: grid;
    gap: 12px;
    scroll-margin-top: 70px;
  }
  .findings {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
  }
  .findings li {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 12px;
    align-items: start;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .findings li:first-child {
    border-top: none;
    padding-top: 0;
  }
  .finding {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .basis {
    overflow-wrap: anywhere;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 10px 32px;
  }
  .share {
    font-size: 14px;
    display: inline-flex;
    gap: 8px;
    align-items: baseline;
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }
  .bullets {
    margin: 0;
    padding-left: 18px;
    display: grid;
    gap: 6px;
    color: var(--text-2);
    font-size: 14px;
  }
  .plain {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .plain li {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: baseline;
    overflow-wrap: anywhere;
  }
  .compare.diverged {
    border-left: 6px solid var(--red);
  }
  .compare ul {
    margin: 0;
    padding-left: 18px;
  }
  details summary {
    cursor: pointer;
    list-style: none;
  }
  details summary h2 {
    display: inline;
  }
  details summary::before {
    content: '▸ ';
    color: var(--text-3);
  }
  details[open] summary::before {
    content: '▾ ';
  }
  @media (max-width: 600px) {
    .toolbar-inner,
    .report {
      padding-left: 16px;
      padding-right: 16px;
    }
    .findings li {
      grid-template-columns: 1fr;
      gap: 6px;
    }
    .headline {
      padding: 20px;
    }
  }
</style>
