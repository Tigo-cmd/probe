<script>
  import BatteryCard from './BatteryCard.svelte';
  import DriveCard from './DriveCard.svelte';
  import FieldList from './FieldList.svelte';
  import Icon from './Icon.svelte';
  import ProvenanceTag from './ProvenanceTag.svelte';
  import Ring from './Ring.svelte';
  import SpecTiles from './SpecTiles.svelte';
  import UseFit from './UseFit.svelte';
  import VerdictBadge from './VerdictBadge.svelte';
  import {
    AXES,
    axisLabel,
    axisSummary,
    BANDS,
    bestForLine,
    coverageLine,
    describeDivergence,
    elevationHint,
    formatGiB,
    machineName,
    ORIGINS,
    osName,
    relativeAge,
    scannedAt,
    sortFindings,
    verdict,
    verdictTone,
  } from './present.js';
  import { compareStored, compareWithSaved, labelStored, saveScanAs, saveTextReportAs } from './api.js';

  let { opened, origin, onback, onrescan = null } = $props();
  const scan = $derived(opened.scan);
  const grade = $derived(opened.grade);
  const assessment = $derived(opened.assessment);
  const condition = $derived(assessment.condition);
  // Writable deriveds: they follow the opened scan and accept local edits.
  let record = $derived(opened.record);
  let label = $derived(opened.record?.label ?? '');
  const head = $derived(verdict(grade.headline));
  const tone = $derived(verdictTone(grade.headline));
  const machine = $derived(machineName(scan.identity));
  const best = $derived(bestForLine(assessment.best_for));

  // The chosen use is a per-viewer convenience: remembered when the browser
  // allows it, otherwise the best match is shown first.
  const USE_KEY = 'probe.selected-use';
  function rememberedUse() {
    try {
      return localStorage.getItem(USE_KEY);
    } catch {
      return null;
    }
  }
  let selectedUse = $state(rememberedUse() ?? opened.assessment.best_for[0] ?? 'everyday');
  $effect(() => {
    try {
      localStorage.setItem(USE_KEY, selectedUse);
    } catch {
      // Storage unavailable: the choice simply is not remembered.
    }
  });

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
        comparison = { title: 'From scan history', comparison: c };
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

  const AREA_ICON = { function: 'drive', battery: 'battery', encumbrance: 'lock' };

  /** The one sentence under the machine name. */
  const sentence = $derived.by(() => {
    if (grade.limiting_axis) {
      const area = condition.areas.find((a) => a.axis === grade.limiting_axis);
      return area?.summary ?? `Limited by ${axisLabel(grade.limiting_axis).toLowerCase()}.`;
    }
    if (grade.headline === 'green') {
      return 'No faults detected in the checks that could run. That lowers risk; it does not clear the machine.';
    }
    return 'Some checks could not run, so this is not a full result.';
  });
</script>

<div class="toolbar">
  <div class="toolbar-inner">
    <button class="back" onclick={onback}><Icon name="back" size={18} /> Scans</button>
    <span class="origin faint" title={origin}>{origin}</span>
    <div class="spacer"></div>
    <button onclick={compare}><Icon name="compare" size={16} /> Compare…</button>
    <button onclick={saveText}><Icon name="doc" size={16} /> Text report…</button>
    <button class="primary" onclick={saveScan}><Icon name="save" size={16} /> Save scan…</button>
  </div>
</div>

<main class="report">
  {#if status}<p class="status" role="status">{status}</p>{/if}
  {#if failure}<p class="status error" role="alert">{failure}</p>{/if}

  <section class="hero card" aria-labelledby="machine-title">
    <div class="hero-main">
      <Ring
        value={condition.score}
        size={196}
        stroke={18}
        {tone}
        label={condition.score === null ? 'Condition not scored' : `Condition score ${condition.score} out of 100`}
      >
        <div class="score">
          <span class="score-num">{condition.score ?? '–'}</span>
          <span class="score-label">{condition.score === null ? 'Not scored' : 'Condition'}</span>
        </div>
      </Ring>
      <div class="hero-text">
        <p class="eyebrow">{machine.vendor || 'Laptop'}</p>
        <h1 id="machine-title">{machine.name}</h1>
        <div class="verdict-line">
          <VerdictBadge value={grade.headline} size="lg" />
          <span class="verdict-words tone-{tone}">
            {condition.band ? BANDS[condition.band] : head.headline}
          </span>
        </div>
        <p class="muted">{sentence}</p>
        <p class="faint small">{coverageLine(condition)}</p>
        {#if best}
          <p class="best"><Icon name="sparkle" size={16} /> {best}</p>
        {/if}
        <p class="faint small">
          Scanned {scannedAt(scan.started_at)} on {osName(scan.os)} · {scan.elevated ? 'full access' : 'limited access'}
        </p>
      </div>
    </div>

    <div class="areas">
      {#each condition.areas as a (a.axis)}
        <a class="area" href="#axis-{a.axis}">
          <Ring value={a.score} size={64} stroke={7} tone={verdictTone(a.verdict)} label="{axisLabel(a.axis)}: {a.score ?? 'not scored'}">
            {#if a.score !== null}
              <span class="area-num">{a.score}</span>
            {:else}
              <span class="faint"><Icon name={AREA_ICON[a.axis]} size={20} /></span>
            {/if}
          </Ring>
          <div class="area-text">
            <p class="area-name">{axisLabel(a.axis)} <VerdictBadge value={a.verdict} /></p>
            <p class="faint small">{a.summary}</p>
          </div>
        </a>
      {/each}
    </div>
  </section>

  {#if !scan.elevated}
    <p class="banner">
      <Icon name="info" size={18} />
      <span>
        <b>Partial scan.</b> It ran without administrator rights, so some reads were skipped.
        {#if onrescan}
          <button class="primary rescan" onclick={onrescan}>Rescan with administrator rights</button>
        {:else}
          {elevationHint(scan.os)}
        {/if}
      </span>
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
    <p class="banner"><Icon name="info" size={18} /> <span>Not kept in scan history: {opened.history_error}</span></p>
  {/if}

  {#if assessment.tips.length}
    <section class="card tips" aria-labelledby="tips-title">
      <h2 id="tips-title" class="section-title">Before you buy</h2>
      <ol>
        {#each assessment.tips as t, i (i)}
          <li><span class="step">{i + 1}</span><span>{t}</span></li>
        {/each}
      </ol>
    </section>
  {/if}

  <UseFit uses={assessment.uses} bind:selected={selectedUse} />

  <SpecTiles specs={assessment.specs} {scan} />

  {#if opened.earlier?.length}
    <section class="card block earlier">
      <div class="block-head">
        <h2 class="section-title">Earlier scans of this laptop</h2>
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

  {#if comparison}
    {@const c = comparison.comparison}
    <section class="card block compare {c.divergences.length ? 'diverged' : ''}">
      <h2 class="section-title">Compared with an earlier scan</h2>
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

  {#if scan.batteries.length}
    <section class="group" aria-labelledby="battery-title">
      <h2 id="battery-title" class="section-title">Battery</h2>
      {#each scan.batteries as b (b.name)}
        <BatteryCard battery={b} />
      {/each}
    </section>
  {/if}

  {#if scan.storage.length}
    <section class="group" aria-labelledby="storage-title">
      <h2 id="storage-title" class="section-title">Storage</h2>
      <div class="drives">
        {#each scan.storage as d (d.name)}
          <DriveCard drive={d} />
        {/each}
      </div>
    </section>
  {/if}

  <section class="group" aria-labelledby="findings-title">
    <div class="block-head">
      <h2 id="findings-title" class="section-title">Every check</h2>
      <span class="faint small">Grader {grade.grader_version} · the overall result is the worst area, never an average</span>
    </div>
    {#each grade.axes as axis (axis.axis)}
      <details class="card findings-card" id="axis-{axis.axis}" open={axis.verdict === 'red' || axis.verdict === 'amber'}>
        <summary>
          <span class="sum-title">
            <Icon name={AREA_ICON[axis.axis]} size={18} />
            <span>{axisLabel(axis.axis)}</span>
            <span class="faint small">{AXES[axis.axis]?.blurb}</span>
          </span>
          <span class="sum-right">
            <span class="faint small">{axisSummary(axis)}</span>
            <VerdictBadge value={axis.verdict} />
            <span class="chev"><Icon name="chevron" size={16} /></span>
          </span>
        </summary>
        {#if axis.findings.length === 0}
          <p class="faint">No checks produced a result for this area.</p>
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
      </details>
    {/each}
  </section>

  <details class="card block">
    <summary class="plain-summary"><h2>About this laptop</h2><span class="chev"><Icon name="chevron" size={16} /></span></summary>
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
    {#if scan.gpus?.length || scan.displays.length || scan.network.length}
      <ul class="plain small">
        {#each scan.gpus ?? [] as g, i (i)}
          <li>
            <span class="faint">Graphics</span>
            {g.name?.value ?? 'Not reported'}
            {#if g.vram_bytes?.value}<span class="muted">{formatGiB(g.vram_bytes.value)} video memory</span>{/if}
            <ProvenanceTag value={g.name?.provenance ?? 'claimed'} />
          </li>
        {/each}
        {#each scan.displays as d, i (i)}
          <li>
            <span class="faint">Display</span>
            {d.manufacturer_id ?? '?'} {d.name ?? ''}
            {#if d.native_width}<span class="muted">{d.native_width}×{d.native_height}</span>{/if}
            {#if d.manufacture_year}<span class="muted">made {d.manufacture_year}</span>{/if}
            <ProvenanceTag value="claimed" />
          </li>
        {/each}
        {#each scan.network as n (n.mac)}
          <li><span class="faint">Network</span> {n.name} <span class="mono muted">{n.mac}</span> <ProvenanceTag value="claimed" /></li>
        {/each}
      </ul>
    {/if}
  </details>

  <section class="two">
    <div class="card block">
      <h2>Check these yourself</h2>
      <p class="faint small">Software cannot assess them. Their absence here is not a pass.</p>
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
      <summary class="plain-summary"><h2>Probe notes ({scan.probe_notes.length})</h2><span class="chev"><Icon name="chevron" size={16} /></span></summary>
      <p class="faint small">Reads that failed or were skipped. Each one is missing evidence, not a clean result.</p>
      <ul class="plain small">
        {#each scan.probe_notes as n, i (i)}
          <li><span class="mono">{n.component}</span> <span class="muted">{n.message}</span></li>
        {/each}
      </ul>
    </details>
  {/if}

  <p class="footer faint tiny">probe {scan.tool_version} · grader {grade.grader_version} · assessment {assessment.version}</p>
</main>

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 5;
    background: color-mix(in srgb, var(--bg) 72%, transparent);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--separator);
  }
  .toolbar-inner {
    max-width: 1080px;
    margin: 0 auto;
    padding: 10px 24px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .toolbar button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    padding: 6px 14px;
  }
  .toolbar .back {
    background: none;
    color: var(--accent);
    padding-left: 4px;
  }
  .origin {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 36ch;
    font-size: 13px;
  }
  .spacer {
    flex: 1;
  }
  .report {
    max-width: 1080px;
    margin: 0 auto;
    padding: 28px 24px 72px;
    display: grid;
    gap: 22px;
  }
  .status {
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--surface);
    box-shadow: var(--shadow);
    overflow-wrap: anywhere;
  }
  .status.error {
    color: var(--red);
    background: var(--red-bg);
    box-shadow: none;
  }

  /* Hero */
  .hero {
    padding: 36px;
    display: grid;
    gap: 30px;
    border-radius: var(--radius-lg);
  }
  .hero-main {
    display: flex;
    gap: 36px;
    align-items: center;
  }
  .score {
    display: grid;
    justify-items: center;
    line-height: 1;
  }
  .score-num {
    font-family: var(--font-display);
    font-size: 64px;
    font-weight: 700;
    letter-spacing: -0.04em;
  }
  .score-label {
    margin-top: 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .hero-text {
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  h1 {
    font-size: 36px;
    line-height: 1.08;
    font-weight: 700;
    letter-spacing: -0.03em;
    overflow-wrap: anywhere;
  }
  .verdict-line {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .verdict-words {
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .tone-green {
    color: var(--green);
  }
  .tone-amber {
    color: var(--amber);
  }
  .tone-red {
    color: var(--red);
  }
  .tone-grey {
    color: var(--grey);
  }
  .best {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    justify-self: start;
    padding: 5px 12px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
    font-size: 14px;
  }
  .areas {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    padding-top: 26px;
    border-top: 1px solid var(--separator);
  }
  .area {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 10px;
    margin: -10px;
    border-radius: 16px;
    color: inherit;
    text-decoration: none;
    transition: background 150ms;
  }
  .area:hover {
    background: var(--surface-2);
  }
  .area-num {
    font-weight: 700;
    font-size: 18px;
    letter-spacing: -0.02em;
  }
  .area-text {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .area-name {
    display: flex;
    gap: 8px;
    align-items: center;
    font-weight: 650;
    font-size: 16px;
  }

  .banner {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    background: var(--amber-bg);
    padding: 14px 16px;
    border-radius: 14px;
  }
  .banner :global(.icon) {
    color: var(--amber);
    margin-top: 2px;
  }
  .rescan {
    margin-left: 8px;
    padding: 4px 14px;
    font-size: 14px;
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
    padding: 7px 12px;
    border-radius: 10px;
    border: 1px solid var(--separator);
    background: var(--surface);
    color: var(--text);
    min-width: 0;
  }

  .tips {
    padding: 26px 28px;
    display: grid;
    gap: 14px;
  }
  .tips ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .tips li {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .step {
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 13px;
    font-weight: 700;
  }

  .group {
    display: grid;
    gap: 14px;
  }
  .drives {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 14px;
  }

  .block {
    padding: 22px 26px;
    display: grid;
    gap: 12px;
    scroll-margin-top: 70px;
  }
  .block-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .earlier-row {
    align-items: center;
  }

  .findings-card {
    padding: 0;
    scroll-margin-top: 70px;
  }
  .findings-card summary,
  .plain-summary {
    list-style: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .findings-card summary {
    padding: 16px 22px;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  .sum-title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 650;
    flex-wrap: wrap;
  }
  .sum-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .chev {
    color: var(--text-3);
    transition: transform 200ms var(--ease);
  }
  details[open] > summary .chev {
    transform: rotate(90deg);
  }
  .findings {
    list-style: none;
    margin: 0;
    padding: 0 22px 14px;
    display: grid;
  }
  .findings li {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 12px;
    align-items: start;
    padding: 12px 0;
    border-top: 1px solid var(--separator);
  }
  .finding {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .basis {
    overflow-wrap: anywhere;
  }
  .findings-card > p {
    padding: 0 22px 16px;
  }

  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 10px 32px;
    margin-top: 6px;
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 14px;
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
    box-shadow: inset 4px 0 0 var(--red-fill), var(--shadow);
  }
  .compare ul {
    margin: 0;
    padding-left: 18px;
  }
  .footer {
    text-align: center;
    margin-top: 8px;
  }

  @media (max-width: 860px) {
    .areas {
      grid-template-columns: 1fr;
      gap: 18px;
    }
    .drives {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 620px) {
    .toolbar-inner,
    .report {
      padding-left: 16px;
      padding-right: 16px;
    }
    .hero {
      padding: 24px 20px;
    }
    .hero-main {
      flex-direction: column;
      align-items: flex-start;
      gap: 20px;
    }
    h1 {
      font-size: 28px;
    }
    .origin {
      display: none;
    }
    .findings li {
      grid-template-columns: 1fr;
      gap: 6px;
    }
    .sum-right .small {
      display: none;
    }
  }
</style>
