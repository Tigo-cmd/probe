<script>
  import History from './History.svelte';
  let {
    onscan,
    onscanlimited,
    onopen,
    privilege = null,
    history = null,
    onopenstored,
    ondeletestored,
    error = '',
    busy = false,
  } = $props();
  // A full scan is offered when the app already has the rights or can ask.
  const full = $derived(!privilege || privilege.elevated || privilege.can_elevate);
</script>

<main class="home">
  <section class="hero card">
    <svg class="mark" viewBox="0 0 1024 1024" aria-hidden="true">
      <rect x="64" y="64" width="896" height="896" rx="200" fill="#1d2a36" />
      <rect x="236" y="300" width="300" height="72" rx="36" fill="#3fb37f" />
      <rect x="236" y="476" width="220" height="72" rx="36" fill="#e3a23b" />
      <rect x="236" y="652" width="140" height="72" rx="36" fill="#e0574f" />
      <circle cx="610" cy="470" r="150" fill="none" stroke="#f4f6f8" stroke-width="56" />
      <line x1="718" y1="578" x2="812" y2="672" stroke="#f4f6f8" stroke-width="64" stroke-linecap="round" />
    </svg>
    <h1>Check this laptop before you buy it</h1>
    <p class="lede muted">
      probe reads the drives, battery and management locks of the machine it runs on, and grades
      what it finds. Everything happens on this laptop. Nothing is sent anywhere.
    </p>
    <div class="actions">
      <button class="primary big" onclick={full ? onscan : onscanlimited} disabled={busy}>Scan this laptop</button>
      <button onclick={onopen} disabled={busy}>Open a saved scan…</button>
    </div>
    {#if privilege?.elevated}
      <p class="access faint">Running with administrator rights, so the scan has full access.</p>
    {:else if privilege?.can_elevate}
      <p class="access faint">
        You will be asked for the {privilege.method} so probe can read drive health, serials and
        firmware tables. It only reads; it changes nothing.
        <button class="link" onclick={onscanlimited} disabled={busy}>Scan without administrator rights</button>
      </p>
    {:else if privilege}
      <p class="access faint">Full scans need administrator rights, which this copy of probe cannot request: {privilege.reason}</p>
    {/if}
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </section>

  {#if history}
    <History {history} onopen={onopenstored} ondelete={ondeletestored} />
  {/if}

  <section class="facts">
    <div class="fact card">
      <h2>What it checks</h2>
      <p class="muted">
        Drive wear and error counters, battery capacity against design, and whether a company,
        Apple Account or tracking agent still has a hold on the machine.
      </p>
    </div>
    <div class="fact card">
      <h2>How to read it</h2>
      <p class="muted">
        Each value is tagged <b>measured</b>, <b>claimed</b> or <b>inferred</b>. Anything it could
        not read is listed as not reported, never as a pass.
      </p>
    </div>
    <div class="fact card">
      <h2>What it cannot tell you</h2>
      <p class="muted">
        Whether the laptop is stolen. No public register of laptop serials exists. Swollen
        batteries, liquid damage and hinge wear need your own eyes.
      </p>
    </div>
  </section>
</main>

<style>
  .home {
    max-width: 880px;
    margin: 0 auto;
    padding: 48px 24px;
    display: grid;
    gap: 20px;
  }
  .hero {
    padding: 40px;
    display: grid;
    gap: 14px;
    justify-items: start;
  }
  .mark {
    width: 56px;
    height: 56px;
    margin-bottom: 6px;
  }
  h1 {
    font-size: 28px;
    line-height: 1.2;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .lede {
    max-width: 60ch;
    font-size: 16px;
  }
  .access {
    font-size: 13px;
    max-width: 62ch;
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    font-weight: 600;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:hover {
    background: none;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 10px;
  }
  .big {
    padding: 10px 22px;
    font-size: 16px;
  }
  .error {
    color: var(--red);
    background: var(--red-bg);
    padding: 8px 12px;
    border-radius: var(--radius-sm);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
  }
  .fact {
    padding: 18px 20px;
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .fact h2 {
    font-size: 14px;
    font-weight: 650;
  }
  .fact p {
    font-size: 14px;
  }
  @media (max-width: 560px) {
    .hero {
      padding: 24px;
    }
    .home {
      padding: 24px 16px;
    }
  }
</style>
