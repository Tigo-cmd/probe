<script>
  // The hands-on tests for one stored scan: a list with each test's latest
  // result, and the running test in place of the list.
  import Icon from './Icon.svelte';
  import Ring from './Ring.svelte';
  import ThemeSwitch from './ThemeSwitch.svelte';
  import KeyboardTest from './tests/KeyboardTest.svelte';
  import PointerTest from './tests/PointerTest.svelte';
  import ScreenTest from './tests/ScreenTest.svelte';
  import SpeakerTest from './tests/SpeakerTest.svelte';
  import PhysicalTest from './tests/PhysicalTest.svelte';
  import { saveHandsOnResult } from './api.js';
  import { OUTCOMES, TESTS } from './handson.js';
  import { machineName } from './present.js';

  let { opened, onback, onsaved } = $props();

  let running = $state(null);
  let failure = $state('');
  const results = $derived(Object.fromEntries((opened.hands_on?.tests ?? []).map((t) => [t.kind, t])));
  const finished = $derived(TESTS.filter((t) => results[t.kind] && results[t.kind].outcome !== 'incomplete').length);
  const machine = $derived(machineName(opened.scan.identity));

  async function complete(result) {
    failure = '';
    try {
      onsaved(await saveHandsOnResult(opened.record.id, result, opened));
      running = null;
    } catch (e) {
      failure = `The result could not be saved: ${e}`;
      running = null;
    }
  }

  const COMPONENTS = { keyboard: KeyboardTest, pointer: PointerTest, screen: ScreenTest, speakers: SpeakerTest, physical: PhysicalTest };
  const Running = $derived(running ? COMPONENTS[running] : null);
</script>

{#if Running}
  <Running os={opened.scan.os} oncomplete={complete} oncancel={() => (running = null)} />
{:else}
  <div class="toolbar">
    <div class="toolbar-inner">
      <button class="back" onclick={onback}><Icon name="back" size={18} /> Report</button>
      <span class="spacer"></span>
      <ThemeSwitch />
    </div>
  </div>
  <main class="hub">
    <header>
      <Ring value={(finished / TESTS.length) * 100} size={84} stroke={9} tone={finished === TESTS.length ? 'green' : 'blue'} label="{finished} of {TESTS.length} tests done">
        <span class="frac">{finished}/{TESTS.length}</span>
      </Ring>
      <div>
        <p class="eyebrow">{machine.vendor} {machine.name}</p>
        <h1>Hands-on tests</h1>
        <p class="muted">About five minutes. Results are kept with this scan and shown in its report.</p>
      </div>
    </header>

    {#if failure}<p class="error" role="alert">{failure}</p>{/if}

    <ul class="list">
      {#each TESTS as t (t.kind)}
        {@const r = results[t.kind]}
        {@const o = r ? OUTCOMES[r.outcome] : null}
        <li class="card">
          <span class="badge"><Icon name={t.icon} size={22} /></span>
          <div class="text">
            <h2>{t.title} <span class="faint small">· about {t.minutes} min</span></h2>
            {#if r}
              <p class="tone-{o.tone} small result">{o.label}: {r.summary}</p>
              {#each r.problems.filter((p) => p !== r.summary).slice(0, 3) as p (p)}<p class="faint small">· {p}</p>{/each}
            {:else}
              <p class="faint small">{t.blurb}</p>
            {/if}
          </div>
          <button class={r ? '' : 'primary'} onclick={() => (running = t.kind)} disabled={!opened.record}>{r ? 'Run again' : 'Start'}</button>
        </li>
      {/each}
    </ul>
    {#if !opened.record}
      <p class="faint small">This scan is not in the history, so test results cannot be kept.</p>
    {/if}

    <section class="card more">
      <h2>More tests coming</h2>
      <p class="faint small">Webcam and microphone, battery drain over ten minutes, a drive read-speed check, a short processor load test for overheating and throttling, Wi-Fi and Bluetooth, and charger wattage.</p>
    </section>
  </main>
{/if}

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 5;
    background: color-mix(in srgb, var(--bg) 72%, transparent);
    backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--separator);
  }
  .toolbar-inner {
    max-width: 860px;
    margin: 0 auto;
    padding: 10px 24px;
    display: flex;
    align-items: center;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    color: var(--accent);
    padding-left: 4px;
  }
  .spacer {
    flex: 1;
  }
  .hub {
    max-width: 860px;
    margin: 0 auto;
    padding: 32px 24px 64px;
    display: grid;
    gap: 22px;
  }
  header {
    display: flex;
    gap: 22px;
    align-items: center;
  }
  h1 {
    font-size: 34px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  .frac {
    font-weight: 700;
    font-size: 18px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 12px;
  }
  .list li {
    padding: 18px 20px;
    display: flex;
    gap: 16px;
    align-items: center;
  }
  .badge {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: #fff;
    background: var(--indigo-fill);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
  }
  .result {
    font-weight: 600;
  }
  .tone-green {
    color: var(--green);
  }
  .tone-red {
    color: var(--red);
  }
  .tone-amber {
    color: var(--amber);
  }
  .more {
    padding: 18px 20px;
    display: grid;
    gap: 4px;
  }
  .error {
    color: var(--red);
    background: var(--red-bg);
    padding: 10px 14px;
    border-radius: 12px;
  }
</style>
