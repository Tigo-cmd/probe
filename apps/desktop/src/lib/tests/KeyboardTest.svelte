<script>
  // Press every key once. Keys are matched by physical position (event.code),
  // so the test works with any keyboard language.
  import { onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import { createKeyTracker, keyboardLayout, keyboardResult, requiredKeys } from '../handson.js';

  let { os, oncomplete, oncancel } = $props();

  const layout = keyboardLayout(os);
  const required = requiredKeys(layout);
  const known = new Set(layout.flat().map((k) => k.code));
  const tracker = createKeyTracker();

  let tick = $state(0); // bumped on every event so the view re-reads the tracker
  let last = $state('');
  let asking = $state(false);
  let held = $state([]);

  const done = $derived((tick, required.filter((k) => tracker.pressed.has(k.code)).length));
  const chatter = $derived((tick, tracker.chattering()));

  function onkeydown(e) {
    // Keep keys from scrolling, switching focus or reloading the page.
    e.preventDefault();
    if (e.repeat) return;
    tracker.keydown(e.code, e.timeStamp, known.has(e.code));
    last = e.code;
    tick++;
  }
  function onkeyup(e) {
    e.preventDefault();
    tracker.keyup(e.code, e.timeStamp);
    tick++;
  }

  onMount(() => {
    window.addEventListener('keydown', onkeydown, true);
    window.addEventListener('keyup', onkeyup, true);
    // Reported only while a key stays down: a stuck key keeps repeating.
    const id = setInterval(() => (held = tracker.held(performance.now()).filter((h) => h.ms > 3000)), 500);
    return () => {
      window.removeEventListener('keydown', onkeydown, true);
      window.removeEventListener('keyup', onkeyup, true);
      clearInterval(id);
    };
  });

  function finish() {
    if (done < required.length) {
      asking = true;
      return;
    }
    oncomplete(keyboardResult(layout, tracker));
  }

  const keyState = (key) => {
    if (key.undetectable) return 'na';
    if (chatter.includes(key.code)) return 'warn';
    return tracker.pressed.has(key.code) ? 'on' : key.optional ? 'opt' : '';
  };
  const labelOf = (code) => layout.flat().find((k) => k.code === code)?.label?.trim() || code;
</script>

<div class="test">
  <header>
    <div>
      <p class="eyebrow">Keyboard test</p>
      <h1>Press every key once</h1>
      <p class="muted">Each key turns green when the computer registers it. Grey keys are optional: many laptops use them for brightness or volume.</p>
    </div>
    <div class="count" aria-live="polite">
      <span class="num">{done}</span><span class="faint">/ {required.length}</span>
    </div>
  </header>

  <div class="board" class:mac={os === 'macos'} aria-label="Keyboard map">
    {#each layout as row, r (r)}
      <div class="row" class:frow={r === 0}>
        {#each row as key (key.code)}
          {@const s = (tick, keyState(key))}
          <span
            class="key {s}"
            class:half={key.half}
            class:last={last === key.code}
            style="flex:{key.w}"
            title={key.undetectable ? 'Fn never reaches software, so it cannot be tested here' : key.code}
          >{key.label}</span>
        {/each}
      </div>
    {/each}
  </div>

  <div class="notes">
    {#if chatter.length}
      <p class="warnline"><Icon name="caution" size={16} /> Registers twice at times: {chatter.map(labelOf).join(', ')}. A worn key switch can do this.</p>
    {/if}
    {#if held.length}
      <p class="warnline"><Icon name="caution" size={16} /> Still held down: {held.map((h) => labelOf(h.code)).join(', ')}. If you are not pressing it, the key may be stuck.</p>
    {/if}
    {#if tick > 0 && [...tracker.extra].length}
      <p class="faint small">Other keys detected: {[...tracker.extra].join(', ')}</p>
    {/if}
    <p class="faint small">Press keys one at a time. System shortcuts such as {os === 'macos' ? 'command-Q or command-Tab' : 'Alt-F4 or Alt-Tab'} still work, so avoid them.</p>
  </div>

  {#if asking}
    <div class="ask card" role="alertdialog" aria-labelledby="kb-ask">
      <p id="kb-ask"><b>{required.length - done} keys have not registered.</b> Did you press them?</p>
      <div class="buttons">
        <button class="primary" onclick={() => oncomplete(keyboardResult(layout, tracker, { broken: true }))}>Yes, they do not work</button>
        <button onclick={() => oncomplete(keyboardResult(layout, tracker))}>I skipped some</button>
        <button class="plain" onclick={() => (asking = false)}>Keep testing</button>
      </div>
    </div>
  {:else}
    <div class="buttons">
      <button class="primary" onclick={finish}>{done === required.length ? 'Done' : 'Finish'}</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    </div>
  {/if}
</div>

<style>
  .test {
    max-width: 1000px;
    margin: 0 auto;
    padding: 32px 24px 48px;
    display: grid;
    gap: 22px;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 24px;
    align-items: flex-end;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .count {
    white-space: nowrap;
    font-family: var(--font-display);
  }
  .num {
    font-size: 44px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  .board {
    display: grid;
    gap: 6px;
    padding: 18px;
    border-radius: 22px;
    background: var(--surface);
    box-shadow: var(--shadow);
    user-select: none;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .frow .key {
    height: 30px;
    font-size: 11px;
  }
  .key {
    position: relative;
    height: 46px;
    min-width: 0;
    display: grid;
    place-items: center;
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: inset 0 -1px 0 rgba(0, 0, 0, 0.12);
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
    overflow: hidden;
    white-space: nowrap;
    transition:
      background 180ms,
      color 180ms,
      transform 120ms;
  }
  .key.half {
    height: 22px;
    align-self: center;
  }
  .key.opt {
    opacity: 0.55;
  }
  .key.na {
    opacity: 0.35;
    border: 1px dashed var(--text-3);
    background: transparent;
  }
  .key.on {
    background: var(--green-fill);
    color: #fff;
  }
  .key.warn {
    background: var(--amber-fill);
    color: #fff;
  }
  .key.last {
    transform: scale(0.94);
  }
  .notes {
    display: grid;
    gap: 6px;
  }
  .warnline {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--amber);
    font-weight: 500;
  }
  .ask {
    padding: 18px 20px;
    display: grid;
    gap: 12px;
  }
  .buttons {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  @media (max-width: 760px) {
    .key {
      height: 34px;
      font-size: 10px;
    }
    .board {
      padding: 10px;
      gap: 4px;
    }
    .row {
      gap: 4px;
    }
  }
</style>
