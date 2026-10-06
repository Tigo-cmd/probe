<main class="scanning" aria-live="polite">
  <div class="card panel">
    <div class="bar" role="progressbar" aria-label="Scanning"><span></span></div>
    <h1>Scanning this laptop…</h1>
    <p class="muted">Reading firmware, drives, battery and management state. This usually takes under a minute.</p>
    <p class="faint">Nothing is written to the drives, and nothing leaves this machine.</p>
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
    max-width: 520px;
    width: 100%;
    padding: 32px;
    display: grid;
    gap: 10px;
  }
  h1 {
    font-size: 20px;
    font-weight: 650;
  }
  .bar {
    height: 4px;
    background: var(--grey-bg);
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 12px;
  }
  .bar span {
    display: block;
    height: 100%;
    width: 35%;
    background: var(--accent);
    border-radius: 2px;
    animation: slide 1.3s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
</style>
