<script>
  // Webcam: a live preview; frames arriving and a picture that is not black
  // are checked automatically, sharpness by the tester. Microphone: a level
  // meter that passes by itself when it hears a voice. Nothing is recorded,
  // and both are switched off when the test ends.
  import { onDestroy, onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { brightness, cameraResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();

  let video;
  let camera = $state({ frames: false, width: 0, height: 0, dark: null, clear: null, error: null });
  let mic = $state({ heard: null, error: null });
  let level = $state(0);
  let peak = 0;
  let streams = [];
  let timers = [];
  let ctx;
  let micWaited = $state(0);

  onMount(async () => {
    try {
      const s = await navigator.mediaDevices.getUserMedia({ video: { width: 1280, height: 720 } });
      streams.push(s);
      video.srcObject = s;
      await video.play();
      const canvas = document.createElement('canvas');
      canvas.width = 64;
      canvas.height = 36;
      const g = canvas.getContext('2d', { willReadFrequently: true });
      timers.push(
        setInterval(() => {
          if (!video.videoWidth) return;
          g.drawImage(video, 0, 0, 64, 36);
          const b = brightness(g.getImageData(0, 0, 64, 36).data);
          camera = { ...camera, frames: true, width: video.videoWidth, height: video.videoHeight, dark: b < 6 };
        }, 500),
      );
    } catch (e) {
      camera = { ...camera, error: e?.name === 'NotAllowedError' ? 'access was refused' : (e?.message ?? String(e)) };
    }
    try {
      const s = await navigator.mediaDevices.getUserMedia({ audio: true });
      streams.push(s);
      ctx = new (window.AudioContext || window.webkitAudioContext)();
      const analyser = ctx.createAnalyser();
      analyser.fftSize = 1024;
      ctx.createMediaStreamSource(s).connect(analyser);
      const data = new Float32Array(analyser.fftSize);
      timers.push(
        setInterval(() => {
          analyser.getFloatTimeDomainData(data);
          let sum = 0;
          for (const v of data) sum += v * v;
          const rms = Math.sqrt(sum / data.length);
          level = Math.min(1, rms * 8);
          peak = Math.max(peak, rms);
          // A voice at arm's length is well above the noise floor.
          if (peak > 0.04 && mic.heard === null) mic = { ...mic, heard: true };
        }, 60),
      );
      timers.push(setInterval(() => (micWaited += 1), 1000));
    } catch (e) {
      mic = { ...mic, error: e?.name === 'NotAllowedError' ? 'access was refused' : (e?.message ?? String(e)) };
    }
  });

  function stop() {
    timers.forEach(clearInterval);
    timers = [];
    streams.forEach((s) => s.getTracks().forEach((t) => t.stop()));
    streams = [];
    if (ctx && ctx.state !== 'closed') ctx.close();
  }
  onDestroy(stop);

  function finish() {
    stop();
    oncomplete(cameraResult(camera, mic));
  }
</script>

<TestFrame eyebrow="Webcam and microphone" title="Can it see and hear you?" lede="Look at the picture, then say something. Nothing is recorded or sent anywhere, and both switch off when you leave this test." wide>
  <div class="grid">
    <div class="panel cam">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={video} muted playsinline></video>
      {#if camera.error}
        <p class="overlay bad"><Icon name="camera" size={20} /> Camera could not start: {camera.error}</p>
      {:else if !camera.frames}
        <p class="overlay wait"><span class="spin"></span> Starting the camera…</p>
      {/if}
    </div>
    <div class="side">
      <div class="panel">
        <p class="status-line {camera.error || camera.dark ? 'bad' : camera.frames ? 'ok' : 'wait'}">
          <Icon name={camera.frames && !camera.dark ? 'check' : 'camera'} size={18} />
          {camera.error ? 'No camera' : camera.dark ? 'Picture is black: is the lens covered?' : camera.frames ? `Picture received, ${camera.width}×${camera.height}` : 'Waiting for a picture'}
        </p>
        {#if camera.frames && !camera.dark}
          <p class="small muted">Is it sharp, with normal colours and no lines?</p>
          <div class="choice">
            <button class:primary={camera.clear === true} onclick={() => (camera.clear = true)}>Looks good</button>
            <button class:primary={camera.clear === false} onclick={() => (camera.clear = false)}>Blurry or faulty</button>
          </div>
        {/if}
      </div>
      <div class="panel">
        <p class="status-line {mic.error || mic.heard === false ? 'bad' : mic.heard ? 'ok' : 'wait'}">
          <Icon name={mic.heard ? 'check' : 'mic'} size={18} />
          {mic.error ? `Microphone could not start: ${mic.error}` : mic.heard ? 'Heard you' : mic.heard === false ? 'Microphone did not react' : 'Say "hello" or clap'}
        </p>
        <div class="meter"><span style="width:{Math.round(level * 100)}%"></span></div>
        {#if mic.heard === null && !mic.error && micWaited > 8}
          <button class="plain" onclick={() => (mic.heard = false)}>It does not react</button>
        {/if}
      </div>
    </div>
  </div>
  {#snippet footer()}
    <button class="primary" onclick={finish}>Done</button>
    <button class="plain" onclick={() => { stop(); oncancel(); }}>Cancel</button>
  {/snippet}
</TestFrame>

<style>
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 16px;
  }
  .cam {
    position: relative;
    padding: 0;
    overflow: hidden;
    aspect-ratio: 16 / 9;
    background: #000;
  }
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scaleX(-1);
    display: block;
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: center;
    color: #fff;
  }
  .side {
    display: grid;
    gap: 16px;
    align-content: start;
  }
  .side .panel {
    display: grid;
    gap: 10px;
  }
  .choice {
    display: flex;
    gap: 8px;
  }
  .meter {
    height: 12px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    background: var(--green-fill);
    transition: width 60ms;
  }
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
