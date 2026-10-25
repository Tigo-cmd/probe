<script>
  // Full-screen colours. Measured automatically: refresh rate (from frame
  // timing), resolution and colour depth. Observed by the tester: defects,
  // optionally marked by clicking where they are.
  import { onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import { setFullscreen } from '../api.js';
  import { refreshRate, SCREEN_COLOURS, SCREEN_ISSUES, screenResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  let phase = $state('intro'); // intro | colours | ask
  let index = $state(0);
  let marks = $state([]);
  let issues = $state([]);
  let hint = $state(true);
  let idle = $state(false);
  let hz = $state(null);

  const colour = $derived(SCREEN_COLOURS[index]);
  const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1;
  const resolution =
    typeof screen !== 'undefined' ? `${Math.round(screen.width * dpr)} × ${Math.round(screen.height * dpr)}` : '';

  onMount(() => {
    // Two seconds of frame timestamps give a steady median.
    const frames = [];
    let id;
    const loop = (t) => {
      frames.push(t);
      if (frames.length < 120) id = requestAnimationFrame(loop);
      else hz = refreshRate(frames);
    };
    id = requestAnimationFrame(loop);
    return () => {
      cancelAnimationFrame(id);
      setFullscreen(false);
    };
  });

  let hintTimer;
  let idleTimer;
  function showHint() {
    hint = true;
    clearTimeout(hintTimer);
    hintTimer = setTimeout(() => (hint = false), 2500);
  }
  function wake() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 1500);
  }

  async function start() {
    await setFullscreen(true);
    phase = 'colours';
    index = 0;
    showHint();
    wake();
  }

  async function advance(step = 1) {
    const n = index + step;
    if (n < 0) return;
    if (n >= SCREEN_COLOURS.length) {
      await setFullscreen(false);
      phase = 'ask';
      if (marks.length) issues = [...new Set([...issues, 'dead_pixels'])];
      return;
    }
    index = n;
    showHint();
  }

  function onkey(e) {
    if (phase !== 'colours') return;
    if (['ArrowRight', ' ', 'Enter'].includes(e.key)) {
      e.preventDefault();
      advance(1);
    } else if (e.key === 'ArrowLeft') {
      advance(-1);
    } else if (e.key === 'Escape') {
      setFullscreen(false);
      phase = 'ask';
    }
  }

  function mark(e) {
    marks = [...marks, { x: e.clientX / window.innerWidth, y: e.clientY / window.innerHeight, colour: colour.name }];
  }

  function toggle(id) {
    issues = issues.includes(id) ? issues.filter((i) => i !== id) : [...issues, id];
  }

  function finish() {
    oncomplete(screenResult(issues, marks, { refresh_hz: hz, resolution, colour_depth: screen?.colorDepth ?? null, device_pixel_ratio: dpr }));
  }
</script>

<svelte:window onkeydown={onkey} />

{#if phase === 'colours'}
  <div
    class="full"
    class:idle
    style="background:{colour.css}"
    onclick={mark}
    onpointermove={wake}
    role="presentation"
  >
    {#each marks.filter((m) => m.colour === colour.name) as m, i (i)}
      <span class="mark" style="left:{m.x * 100}%;top:{m.y * 100}%"></span>
    {/each}
    <div class="hud" class:shown={hint}>
      <b>{colour.name}</b> · {index + 1} of {SCREEN_COLOURS.length}<br />
      {colour.look}<br />
      <span class="dim">Click a bad spot to mark it · → or space for next · Esc to stop</span>
    </div>
    <div class="nav" class:shown={!idle}>
      <button onclick={(e) => { e.stopPropagation(); advance(-1); }} disabled={index === 0}>Back</button>
      <button class="primary" onclick={(e) => { e.stopPropagation(); advance(1); }}>Next</button>
    </div>
  </div>
{:else}
  <div class="test">
    <p class="eyebrow">Screen test</p>
    {#if phase === 'intro'}
      <h1>Look for dead pixels, patches and lines</h1>
      <p class="muted">
        The screen fills with {SCREEN_COLOURS.length} colours, one at a time. Look closely at each, from
        straight on and from an angle. Click any spot that looks wrong to mark it.
      </p>
      <div class="measured card">
        <div><span class="faint small">Refresh rate</span><b>{hz ? `${hz} Hz` : 'Measuring…'}</b></div>
        <div><span class="faint small">Resolution</span><b>{resolution}</b></div>
        <div><span class="faint small">Scaling</span><b>{Math.round(dpr * 100)}%</b></div>
      </div>
      <p class="faint small">Turn the brightness up to full first. Wipe the screen so dust is not mistaken for a pixel.</p>
      <div class="buttons">
        <button class="primary" onclick={start}><Icon name="display" size={16} /> Start full screen</button>
        <button class="plain" onclick={oncancel}>Cancel</button>
      </div>
    {:else}
      <h1>What did you see?</h1>
      <p class="muted">Pick anything you noticed{marks.length ? `. You marked ${marks.length} ${marks.length === 1 ? 'spot' : 'spots'}.` : '.'}</p>
      <div class="chips">
        {#each SCREEN_ISSUES as i (i.id)}
          <button class="chip" class:on={issues.includes(i.id)} aria-pressed={issues.includes(i.id)} onclick={() => toggle(i.id)}>
            {#if issues.includes(i.id)}<Icon name="check" size={14} stroke={2.4} />{/if}
            {i.label}
          </button>
        {/each}
      </div>
      <div class="buttons">
        <button class="primary" onclick={finish}>{issues.length ? 'Save these problems' : 'No problems seen'}</button>
        <button onclick={start}>Look again</button>
        <button class="plain" onclick={oncancel}>Cancel</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .full {
    position: fixed;
    inset: 0;
    z-index: 50;
    cursor: crosshair;
  }
  .full.idle {
    cursor: none;
  }
  .mark {
    position: absolute;
    width: 34px;
    height: 34px;
    margin: -17px 0 0 -17px;
    border-radius: 50%;
    border: 3px solid #ff2d55;
    box-shadow: 0 0 0 2px #fff;
    pointer-events: none;
  }
  .hud {
    position: absolute;
    left: 50%;
    top: 32px;
    transform: translateX(-50%);
    max-width: 520px;
    padding: 12px 18px;
    border-radius: 14px;
    background: rgba(28, 28, 30, 0.82);
    color: #fff;
    font-size: 14px;
    text-align: center;
    backdrop-filter: blur(12px);
    opacity: 0;
    transition: opacity 400ms;
    pointer-events: none;
  }
  .hud.shown {
    opacity: 1;
  }
  .dim {
    opacity: 0.7;
    font-size: 12px;
  }
  .nav {
    position: absolute;
    bottom: 28px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 8px;
    opacity: 0;
    transition: opacity 300ms;
  }
  .nav.shown {
    opacity: 1;
  }
  .test {
    max-width: 760px;
    margin: 0 auto;
    padding: 40px 24px;
    display: grid;
    gap: 16px;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .measured {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: 18px 20px;
    gap: 12px;
  }
  .measured div {
    display: grid;
    gap: 2px;
  }
  .measured b {
    font-size: 20px;
    font-family: var(--font-display);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--separator);
  }
  .chip.on {
    background: var(--red-bg);
    color: var(--red);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--red) 40%, transparent);
  }
  .buttons {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 6px;
  }
</style>
