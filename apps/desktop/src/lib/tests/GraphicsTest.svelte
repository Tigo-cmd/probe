<script>
  // A minute of heavy 3D drawing. Watches three things: the frame rate
  // falling as the graphics chip heats up, test images coming back with
  // wrong pixels (a sign of faulty graphics memory), and the driver
  // crashing (WebGL reports the context as lost).
  import { onDestroy, onMount } from 'svelte';
  import Icon from '../Icon.svelte';
  import TestFrame from './TestFrame.svelte';
  import { graphicsResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();
  const SECONDS = 60;
  const CHECK_SIZE = 64;

  let phase = $state('intro');
  let perSecond = $state([]);
  let glitches = $state(0);
  let renderer = $state('');
  let canvas = $state();
  let gl = null;
  let heavy = null;
  let pattern = null;
  let lost = false;
  let raf = 0;
  let timer = 0;
  let frames = 0;
  let t0 = 0;

  // Heavy: many iterations of trigonometry per pixel.
  const HEAVY_FS = `#version 300 es
precision highp float;
uniform float u_t;
uniform vec2 u_res;
out vec4 o;
void main() {
  vec2 p = (gl_FragCoord.xy * 2.0 - u_res) / u_res.y;
  vec3 c = vec3(0.0);
  for (int i = 0; i < 96; i++) {
    float fi = float(i);
    p = vec2(p.x * cos(0.11 + u_t * 0.1) - p.y * sin(0.11), p.x * sin(0.11) + p.y * cos(0.11 + fi * 0.001));
    p = abs(p) / dot(p, p) - vec2(0.9 + 0.05 * sin(u_t), 0.6);
    c += vec3(0.4 + 0.4 * sin(fi * 0.21 + u_t), 0.3 + 0.3 * sin(fi * 0.13), 0.7) * exp(-6.0 * length(p)) * 0.14;
  }
  o = vec4(c, 1.0);
}`;
  // Exact: integer arithmetic every graphics chip computes identically.
  const PATTERN_FS = `#version 300 es
precision highp float;
precision highp int;
out vec4 o;
void main() {
  ivec2 q = ivec2(gl_FragCoord.xy);
  int v = (q.x * 7 + q.y * 13 + (q.x ^ q.y) * 3) & 255;
  o = vec4(float(v) / 255.0, float((v * 5) & 255) / 255.0, float(255 - v) / 255.0, 1.0);
}`;
  const VS = `#version 300 es
in vec2 a;
void main() { gl_Position = vec4(a, 0.0, 1.0); }`;

  function program(src) {
    const make = (type, text) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, text);
      gl.compileShader(s);
      return s;
    };
    const p = gl.createProgram();
    gl.attachShader(p, make(gl.VERTEX_SHADER, VS));
    gl.attachShader(p, make(gl.FRAGMENT_SHADER, src));
    gl.bindAttribLocation(p, 0, 'a');
    gl.linkProgram(p);
    return gl.getProgramParameter(p, gl.LINK_STATUS) ? p : null;
  }

  /** "ANGLE (NVIDIA, NVIDIA GeForce RTX 3050 Direct3D11 vs_5_0 ps_5_0, D3D11)" -> the chip name. */
  function tidyRenderer(r) {
    const m = /^ANGLE \((.*)\)$/.exec(r ?? '');
    const name = m ? (m[1].split(', ')[1] ?? m[1]) : (r ?? '');
    return name.replace(/\s+(Direct3D|vs_|ps_|\(0x).*$/, '').trim();
  }

  function expected(x, y) {
    const v = (x * 7 + y * 13 + (x ^ y) * 3) & 255;
    return [v, (v * 5) & 255, 255 - v];
  }

  onMount(() => {
    gl = canvas?.getContext('webgl2', { antialias: false, preserveDrawingBuffer: false, powerPreference: 'high-performance' });
    if (!gl) return;
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    renderer = tidyRenderer(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
    canvas.addEventListener('webglcontextlost', (e) => {
      e.preventDefault();
      lost = true;
      if (phase === 'running') finish();
    });
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    heavy = program(HEAVY_FS);
    pattern = program(PATTERN_FS);
  });

  /** Draw the exact pattern into a corner and compare every pixel. */
  function checkPattern() {
    gl.useProgram(pattern);
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(0, 0, CHECK_SIZE, CHECK_SIZE);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    const px = new Uint8Array(CHECK_SIZE * CHECK_SIZE * 4);
    gl.readPixels(0, 0, CHECK_SIZE, CHECK_SIZE, gl.RGBA, gl.UNSIGNED_BYTE, px);
    gl.disable(gl.SCISSOR_TEST);
    let wrong = 0;
    for (let y = 0; y < CHECK_SIZE; y++) {
      for (let x = 0; x < CHECK_SIZE; x++) {
        const i = (y * CHECK_SIZE + x) * 4;
        const [r, g, b] = expected(x, y);
        if (Math.abs(px[i] - r) > 1 || Math.abs(px[i + 1] - g) > 1 || Math.abs(px[i + 2] - b) > 1) wrong++;
      }
    }
    if (wrong > 0) glitches += 1;
  }

  function frame(now) {
    if (phase !== 'running' || lost) return;
    const t = (now - t0) / 1000;
    gl.useProgram(heavy);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.uniform1f(gl.getUniformLocation(heavy, 'u_t'), t);
    gl.uniform2f(gl.getUniformLocation(heavy, 'u_res'), canvas.width, canvas.height);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    frames++;
    raf = requestAnimationFrame(frame);
  }

  function start() {
    if (!gl || !heavy || !pattern) {
      oncomplete(graphicsResult({ unavailable: true, perSecond: [], glitches: 0, lost: false, renderer }));
      return;
    }
    phase = 'running';
    perSecond = [];
    frames = 0;
    t0 = performance.now();
    raf = requestAnimationFrame(frame);
    timer = setInterval(() => {
      perSecond = [...perSecond, frames];
      frames = 0;
      if (perSecond.length % 5 === 0) checkPattern();
      if (perSecond.length >= SECONDS) finish();
    }, 1000);
  }

  function stopAll() {
    cancelAnimationFrame(raf);
    clearInterval(timer);
  }

  function finish() {
    if (phase !== 'running') return;
    phase = 'done';
    stopAll();
    oncomplete(graphicsResult({ perSecond, glitches, lost, renderer, seconds: perSecond.length }));
  }

  onDestroy(stopAll);
  const peak = $derived(Math.max(1, ...perSecond));
</script>

<TestFrame
  eyebrow="Graphics under load"
  title={phase === 'intro' ? 'Does the graphics chip stay stable?' : `Drawing… ${SECONDS - perSecond.length} s`}
  lede={phase === 'intro'
    ? `For ${SECONDS} seconds probe draws a heavy 3D scene and checks test images pixel by pixel. The fan will speed up; that is expected. Plug the charger in for a fair result.`
    : ''}
  wide
>
  <canvas bind:this={canvas} width="960" height="400" class:hidden={phase === 'intro'}></canvas>
  {#if phase !== 'intro'}
    <div class="chart" aria-label="Frames each second">
      {#each Array(SECONDS) as _, i (i)}
        <span style="height:{perSecond[i] ? (perSecond[i] / peak) * 100 : 0}%"></span>
      {/each}
    </div>
    <div class="stat-row">
      <div class="stat"><span class="faint small">Frames a second</span><b>{perSecond.at(-1) ?? '–'}</b></div>
      <div class="stat"><span class="faint small">Glitched test images</span><b class:bad={glitches > 0}>{glitches}</b></div>
      <div class="stat"><span class="faint small">Graphics</span><span class="renderer">{renderer || 'Not reported'}</span></div>
    </div>
  {/if}
  {#snippet footer()}
    {#if phase === 'intro'}
      <button class="primary" onclick={start}><Icon name="graphics" size={16} /> Start</button>
      <button class="plain" onclick={oncancel}>Cancel</button>
    {:else}
      <button onclick={finish}>Stop now</button>
    {/if}
  {/snippet}
</TestFrame>

<style>
  canvas {
    width: 100%;
    height: 260px;
    border-radius: 16px;
    background: #000;
  }
  canvas.hidden {
    display: none;
  }
  .chart {
    height: 90px;
    display: flex;
    align-items: flex-end;
    gap: 2px;
  }
  .chart span {
    flex: 1;
    border-radius: 3px 3px 0 0;
    background: var(--indigo-fill, var(--blue-fill));
    transition: height 300ms;
  }
  .bad {
    color: var(--red);
  }
  .renderer {
    font-size: 15px;
    font-weight: 600;
    line-height: 1.3;
  }
</style>
