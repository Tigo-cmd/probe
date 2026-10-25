<script>
  // Speakers: a chime from one side at a time, in a random order, and the
  // tester says where they heard it, so the answer can be checked. Then a
  // low-to-high sweep, where a damaged speaker buzzes or rattles.
  import { onDestroy } from 'svelte';
  import Icon from '../Icon.svelte';
  import { speakerResult, speakerRounds } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  const rounds = speakerRounds();
  let round = $state(-1); // -1 intro, 0..2 sides, 3 sweep
  let answers = $state({});
  let playing = $state(false);
  let failure = $state('');
  let ctx;

  function audio() {
    ctx ??= new (window.AudioContext || window.webkitAudioContext)();
    if (ctx.state === 'suspended') ctx.resume();
    return ctx;
  }

  /** A two-note chime panned to `side`. */
  function chime(side) {
    const a = audio();
    const pan = a.createStereoPanner();
    pan.pan.value = side === 'left' ? -1 : side === 'right' ? 1 : 0;
    pan.connect(a.destination);
    const t = a.currentTime + 0.05;
    [659.25, 987.77, 659.25, 987.77].forEach((f, i) => {
      const o = a.createOscillator();
      const g = a.createGain();
      o.type = 'sine';
      o.frequency.value = f;
      const at = t + i * 0.32;
      g.gain.setValueAtTime(0, at);
      g.gain.linearRampToValueAtTime(0.35, at + 0.02);
      g.gain.exponentialRampToValueAtTime(0.001, at + 0.3);
      o.connect(g).connect(pan);
      o.start(at);
      o.stop(at + 0.32);
    });
    return 1.4;
  }

  /** 80 Hz to 12 kHz over four seconds, both sides. */
  function sweep() {
    const a = audio();
    const o = a.createOscillator();
    const g = a.createGain();
    const t = a.currentTime + 0.05;
    o.frequency.setValueAtTime(80, t);
    o.frequency.exponentialRampToValueAtTime(12000, t + 4);
    g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(0.25, t + 0.1);
    g.gain.setValueAtTime(0.25, t + 3.8);
    g.gain.linearRampToValueAtTime(0, t + 4);
    o.connect(g).connect(a.destination);
    o.start(t);
    o.stop(t + 4.05);
    return 4.1;
  }

  async function play() {
    failure = '';
    try {
      playing = true;
      const seconds = round < 3 ? chime(rounds[round]) : sweep();
      setTimeout(() => (playing = false), seconds * 1000);
    } catch (e) {
      playing = false;
      failure = `Sound could not play: ${e?.message ?? e}`;
    }
  }

  function begin() {
    round = 0;
    play();
  }

  function answer(heard) {
    answers = { ...answers, [rounds[round]]: heard };
    round++;
    setTimeout(play, 350);
  }

  onDestroy(() => ctx?.close?.());
</script>

<div class="test">
  <p class="eyebrow">Speaker test {round >= 0 ? `· ${Math.min(round + 1, 4)} of 4` : ''}</p>
  {#if round < 0}
    <h1>Can you hear both sides?</h1>
    <p class="muted">
      A chime plays from one speaker at a time, in a random order. Say where you heard it. Unplug
      headphones and set the volume to about half.
    </p>
    <div class="buttons">
      <button class="primary" onclick={begin}><Icon name="speaker" size={16} /> Play the first sound</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    </div>
  {:else if round < 3}
    <h1>Where did you hear it?</h1>
    <div class="wave" class:on={playing} aria-hidden="true">
      {#each Array(9) as _, i (i)}<span style="animation-delay:{i * 70}ms"></span>{/each}
    </div>
    <div class="answers">
      {#each [['left', 'Left'], ['right', 'Right'], ['both', 'Both sides'], ['nothing', 'Nothing']] as [v, label] (v)}
        <button class="answer" onclick={() => answer(v)} disabled={playing}>{label}</button>
      {/each}
    </div>
    <button class="plain" onclick={play} disabled={playing}>Play again</button>
  {:else}
    <h1>Did it buzz or rattle?</h1>
    <p class="muted">A tone rises from low to high. A clean speaker sounds smooth; a damaged one buzzes, crackles or rattles, often on the low notes.</p>
    <div class="wave" class:on={playing} aria-hidden="true">
      {#each Array(9) as _, i (i)}<span style="animation-delay:{i * 70}ms"></span>{/each}
    </div>
    <div class="answers">
      <button class="answer" onclick={() => oncomplete(speakerResult(answers, false))} disabled={playing}>Sounded clean</button>
      <button class="answer" onclick={() => oncomplete(speakerResult(answers, true))} disabled={playing}>It buzzed or rattled</button>
    </div>
    <button class="plain" onclick={play} disabled={playing}>Play again</button>
  {/if}
  {#if failure}<p class="error small" role="alert">{failure}</p>{/if}
</div>

<style>
  .test {
    max-width: 640px;
    margin: 0 auto;
    padding: 56px 24px;
    display: grid;
    gap: 18px;
    justify-items: center;
    text-align: center;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .wave {
    display: flex;
    gap: 6px;
    align-items: center;
    height: 80px;
  }
  .wave span {
    width: 8px;
    height: 14px;
    border-radius: 999px;
    background: var(--track);
  }
  .wave.on span {
    background: var(--accent);
    animation: pulse 700ms ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      height: 72px;
    }
  }
  .answers {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 10px;
    width: 100%;
  }
  .answer {
    padding: 14px;
    font-size: 16px;
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .buttons {
    display: flex;
    gap: 10px;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
  }
  .error {
    color: var(--red);
  }
</style>
