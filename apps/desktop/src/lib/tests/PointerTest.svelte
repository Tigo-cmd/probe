<script>
  // Touchpad and mouse. Each step watches for its own event and moves on by
  // itself when it sees it; "This does not work" records a fault and
  // "Skip" leaves the step unchecked. The report rate is measured from the
  // movement events, and the last step watches for a pointer that moves
  // with nobody touching it (a failing touchpad's "ghost" input).
  import { onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import { coverage, POINTER_STEPS, pointerResult, reportRate } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  const COLS = 12;
  const ROWS = 7;

  let index = $state(0);
  let steps = $state({});
  let visited = $state(new Set());
  let times = [];
  let touch = $state(false);
  let flash = $state(false);
  let scrolled = $state({ up: 0, down: 0, left: 0, right: 0 });
  let drag = $state({ x: 0.15, y: 0.5, on: false, inside: false });
  let still = $state({ phase: 'wait', moved: 0, left: 3 });
  let pad;

  const step = $derived(POINTER_STEPS[index]);
  const covered = $derived(coverage(visited, COLS, ROWS));

  function next(status) {
    steps[step.id] = status;
    flash = status === 'done';
    setTimeout(() => (flash = false), 450);
    if (index < POINTER_STEPS.length - 1) {
      index++;
      if (POINTER_STEPS[index].id === 'still') startStill();
    } else finish();
  }

  function finish() {
    oncomplete(pointerResult(steps, { rate_hz: reportRate(times), touchscreen: touch }));
  }

  // --- sweep
  function onmove(e) {
    if (e.pointerType === 'touch') touch = true;
    if (step.id === 'sweep') {
      const events = e.getCoalescedEvents?.() ?? [e];
      for (const ev of events) times.push(ev.timeStamp);
      if (times.length > 400) times = times.slice(-400);
      const r = pad.getBoundingClientRect();
      const cx = Math.floor(((e.clientX - r.left) / r.width) * COLS);
      const cy = Math.floor(((e.clientY - r.top) / r.height) * ROWS);
      if (cx >= 0 && cy >= 0 && cx < COLS && cy < ROWS && !visited.has(cy * COLS + cx)) {
        visited = new Set(visited).add(cy * COLS + cx);
        if (coverage(visited, COLS, ROWS) >= 0.92) next('done');
      }
    }
    if (step.id === 'drag' && drag.on) {
      const r = pad.getBoundingClientRect();
      const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
      const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
      drag = { ...drag, x, y, inside: x > 0.7 && x < 0.95 && y > 0.25 && y < 0.75 };
    }
    if (step.id === 'still' && still.phase === 'watch') still.moved += Math.hypot(e.movementX ?? 0, e.movementY ?? 0);
  }

  // --- clicks
  function ondown(e) {
    if (e.pointerType === 'touch') touch = true;
    if (step.id === 'click' && e.button === 0 && e.target.closest('.target')) next('done');
    if (step.id === 'right' && e.button === 2 && e.target.closest('.target')) next('done');
    if (step.id === 'drag' && e.button === 0 && e.target.closest('.dot')) {
      drag = { ...drag, on: true };
      e.target.setPointerCapture?.(e.pointerId);
    }
  }
  function onup() {
    if (step.id === 'drag' && drag.on) {
      if (drag.inside) next('done');
      else drag = { ...drag, on: false };
    }
  }
  function ondbl(e) {
    if (step.id === 'double' && e.target.closest('.target')) next('done');
  }

  // --- scroll
  function onwheel(e) {
    if (step.id !== 'scroll') return;
    e.preventDefault();
    const s = { ...scrolled };
    if (e.deltaY < 0) s.up += -e.deltaY;
    if (e.deltaY > 0) s.down += e.deltaY;
    if (e.deltaX < 0) s.left += -e.deltaX;
    if (e.deltaX > 0) s.right += e.deltaX;
    scrolled = s;
    if (Object.values(s).every((v) => v > 120)) next('done');
  }

  // --- hands off
  let timer;
  function startStill() {
    still = { phase: 'wait', moved: 0, left: 3 };
    clearInterval(timer);
    // Two seconds to lift the hands, then three seconds of watching.
    setTimeout(() => {
      still = { phase: 'watch', moved: 0, left: 3 };
      timer = setInterval(() => {
        still.left -= 1;
        if (still.left <= 0) {
          clearInterval(timer);
          // More than a few pixels of travel with nobody touching is drift.
          next(still.moved > 8 ? 'failed' : 'done');
        }
      }, 1000);
    }, 2000);
  }

  onMount(() => {
    const block = (e) => step.id === 'right' && e.preventDefault();
    window.addEventListener('contextmenu', block);
    return () => {
      window.removeEventListener('contextmenu', block);
      clearInterval(timer);
    };
  });

  const scrollPct = (v) => Math.min(100, Math.round((v / 120) * 100));
</script>

<div class="test">
  <header>
    <div>
      <p class="eyebrow">Touchpad and mouse · step {index + 1} of {POINTER_STEPS.length}</p>
      <h1>{step.title}</h1>
      <p class="muted">{step.hint}</p>
    </div>
    <ol class="dots" aria-hidden="true">
      {#each POINTER_STEPS as s, i (s.id)}
        <li class:done={steps[s.id] === 'done'} class:failed={steps[s.id] === 'failed'} class:now={i === index}></li>
      {/each}
    </ol>
  </header>

  <div
    class="pad"
    class:flash
    bind:this={pad}
    onpointermove={onmove}
    onpointerdown={ondown}
    onpointerup={onup}
    ondblclick={ondbl}
    onwheel={onwheel}
    role="application"
    aria-label="Test area"
  >
    {#if step.id === 'sweep'}
      <div class="grid" style="grid-template-columns:repeat({COLS},1fr)">
        {#each Array(COLS * ROWS) as _, i (i)}
          <span class:on={visited.has(i)}></span>
        {/each}
      </div>
      <span class="badge-pct">{Math.round(covered * 100)}%</span>
    {:else if step.id === 'click' || step.id === 'right' || step.id === 'double'}
      <button class="target" aria-label={step.title} tabindex="-1">
        <Icon name="pointer" size={28} />
      </button>
    {:else if step.id === 'drag'}
      <div class="box" class:inside={drag.inside}>Drop here</div>
      <span class="dot" style="left:{drag.x * 100}%;top:{drag.y * 100}%"></span>
    {:else if step.id === 'scroll'}
      <div class="scrolls">
        {#each [['up', '↑'], ['down', '↓'], ['left', '←'], ['right', '→']] as [dir, arrow] (dir)}
          <div class="scroll">
            <span>{arrow}</span>
            <span class="bar"><span style="width:{scrollPct(scrolled[dir])}%"></span></span>
          </div>
        {/each}
      </div>
    {:else if step.id === 'still'}
      <div class="still">
        {#if still.phase === 'wait'}
          <p class="big">Hands off…</p>
        {:else}
          <p class="big">{still.left}</p>
          <p class="faint">Watching for movement: {Math.round(still.moved)} px</p>
        {/if}
      </div>
    {/if}
  </div>

  <div class="buttons">
    {#if step.id !== 'still'}
      <button onclick={() => next('failed')}>This does not work</button>
      <button class="plain" onclick={() => next('skipped')}>Skip</button>
    {/if}
    <span class="spacer"></span>
    {#if touch}<span class="faint small">Touchscreen detected</span>{/if}
    <button class="plain" onclick={oncancel}>Cancel</button>
  </div>
</div>

<style>
  .test {
    max-width: 1000px;
    margin: 0 auto;
    padding: 32px 24px 48px;
    display: grid;
    gap: 20px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 20px;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .dots {
    display: flex;
    gap: 6px;
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .dots li {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--track);
  }
  .dots li.now {
    background: var(--accent);
  }
  .dots li.done {
    background: var(--green-fill);
  }
  .dots li.failed {
    background: var(--red-fill);
  }
  .pad {
    position: relative;
    height: min(56vh, 460px);
    border-radius: 22px;
    background: var(--surface);
    box-shadow: var(--shadow);
    overflow: hidden;
    display: grid;
    place-items: center;
    touch-action: none;
    user-select: none;
    transition: box-shadow 300ms;
  }
  .pad.flash {
    box-shadow:
      0 0 0 4px var(--green-fill),
      var(--shadow);
  }
  .grid {
    position: absolute;
    inset: 0;
    display: grid;
    gap: 3px;
    padding: 3px;
  }
  .grid span {
    border-radius: 6px;
    background: var(--surface-2);
    transition: background 200ms;
  }
  .grid span.on {
    background: color-mix(in srgb, var(--blue-fill) 75%, transparent);
  }
  .badge-pct {
    position: relative;
    font-family: var(--font-display);
    font-size: 48px;
    font-weight: 700;
    color: var(--text);
    pointer-events: none;
    mix-blend-mode: difference;
    color: #fff;
  }
  .target {
    width: 120px;
    height: 120px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: grid;
    place-items: center;
    box-shadow: 0 10px 30px color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .box {
    position: absolute;
    left: 70%;
    right: 5%;
    top: 25%;
    bottom: 25%;
    border: 3px dashed var(--text-3);
    border-radius: 18px;
    display: grid;
    place-items: center;
    color: var(--text-3);
    font-weight: 600;
  }
  .box.inside {
    border-color: var(--green-fill);
    color: var(--green);
  }
  .dot {
    position: absolute;
    width: 54px;
    height: 54px;
    margin: -27px 0 0 -27px;
    border-radius: 50%;
    background: var(--indigo-fill);
    cursor: grab;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.2);
  }
  .scrolls {
    display: grid;
    gap: 14px;
    width: min(420px, 80%);
  }
  .scroll {
    display: grid;
    grid-template-columns: 24px 1fr;
    align-items: center;
    gap: 12px;
    font-size: 20px;
  }
  .bar {
    height: 12px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--teal-fill);
    border-radius: 999px;
    transition: width 120ms;
  }
  .still {
    text-align: center;
  }
  .big {
    font-family: var(--font-display);
    font-size: 64px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  .buttons {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
</style>
