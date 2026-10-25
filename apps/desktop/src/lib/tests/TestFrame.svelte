<script>
  // Shared layout for a hands-on test: heading, body, then the buttons.
  let { eyebrow, title, lede = '', wide = false, children, footer } = $props();
</script>

<div class="frame" class:wide>
  <p class="eyebrow">{eyebrow}</p>
  <h1>{title}</h1>
  {#if lede}<p class="muted lede">{lede}</p>{/if}
  <div class="body">{@render children?.()}</div>
  {#if footer}<div class="footer">{@render footer()}</div>{/if}
</div>

<style>
  .frame {
    max-width: 760px;
    margin: 0 auto;
    padding: 36px 24px 48px;
    display: grid;
    gap: 14px;
  }
  .frame.wide {
    max-width: 980px;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  .lede {
    max-width: 62ch;
  }
  .body {
    display: grid;
    gap: 16px;
    margin-top: 6px;
  }
  .footer {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .frame :global(.panel) {
    background: var(--surface);
    border-radius: 20px;
    box-shadow: var(--shadow);
    padding: 22px;
  }
  .frame :global(.stat-row) {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 12px;
  }
  .frame :global(.stat) {
    display: grid;
    gap: 2px;
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .frame :global(.stat b) {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .frame :global(.status-line) {
    display: flex;
    gap: 10px;
    align-items: center;
    font-weight: 600;
  }
  .frame :global(.ok) {
    color: var(--green);
  }
  .frame :global(.bad) {
    color: var(--red);
  }
  .frame :global(.wait) {
    color: var(--text-3);
  }
  .frame :global(.spin) {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2.5px solid var(--track);
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
