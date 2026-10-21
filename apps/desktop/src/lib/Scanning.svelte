<script>
  let { asking = '' } = $props();
  const READING = ['Firmware and identity', 'Drive health and wear', 'Battery capacity', 'Locks and management', 'Processor, memory and graphics'];
</script>

<main class="scanning" aria-live="polite">
  <div class="panel">
    <svg class="rings" viewBox="0 0 120 120" aria-hidden="true">
      <g fill="none" stroke-linecap="round" stroke-width="10">
        <circle class="track" cx="60" cy="60" r="50" />
        <circle class="arc a" cx="60" cy="60" r="50" stroke-dasharray="90 225" />
        <circle class="track" cx="60" cy="60" r="36" />
        <circle class="arc b" cx="60" cy="60" r="36" stroke-dasharray="60 166" />
        <circle class="track" cx="60" cy="60" r="22" />
        <circle class="arc c" cx="60" cy="60" r="22" stroke-dasharray="40 98" />
      </g>
    </svg>
    <h1>Scanning this laptop</h1>
    <p class="muted">This usually takes under a minute.</p>
    {#if asking}
      <p class="ask">If the system asks for the {asking}, enter it to give the scan full access, or cancel to choose a limited scan.</p>
    {/if}
    <ul aria-label="What is being read">
      {#each READING as r, i (r)}
        <li style="animation-delay:{i * 180}ms">{r}</li>
      {/each}
    </ul>
    <p class="faint small">Nothing is written to the drives, and nothing leaves this laptop.</p>
  </div>
</main>

<style>
  .scanning {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .panel {
    max-width: 460px;
    width: 100%;
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 10px;
  }
  .rings {
    width: 132px;
    height: 132px;
    margin-bottom: 12px;
  }
  .track {
    stroke: var(--track);
  }
  .arc {
    transform-origin: 60px 60px;
    animation: spin 1.6s cubic-bezier(0.5, 0.15, 0.5, 0.85) infinite;
  }
  .a {
    stroke: var(--green-fill);
  }
  .b {
    stroke: var(--blue-fill);
    animation-duration: 1.25s;
    animation-direction: reverse;
  }
  .c {
    stroke: var(--amber-fill);
    animation-duration: 1s;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  h1 {
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .ask {
    background: var(--accent-soft);
    color: var(--text);
    padding: 10px 14px;
    border-radius: 12px;
    font-size: 14px;
  }
  ul {
    list-style: none;
    margin: 14px 0 6px;
    padding: 0;
    display: grid;
    gap: 6px;
    color: var(--text-2);
    font-size: 15px;
  }
  li {
    opacity: 0;
    animation: rise 500ms var(--ease) forwards;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    li {
      opacity: 1;
    }
  }
</style>
