<script>
  import History from './History.svelte';
  import Icon from './Icon.svelte';
  import TeamCard from './TeamCard.svelte';
  import ThemeSwitch from './ThemeSwitch.svelte';
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
    team = null,
    onteamchange = () => {},
    onsynced = () => {},
    onwipeguide = null,
  } = $props();
  // A full scan is offered when the app already has the rights or can ask.
  const full = $derived(!privilege || privilege.elevated || privilege.can_elevate);
</script>

<main class="home">
  <div class="topbar"><ThemeSwitch /></div>
  <section class="hero">
    <svg class="mark" viewBox="0 0 120 120" aria-hidden="true">
      <defs>
        <linearGradient id="mark-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#2c2c2e" />
          <stop offset="1" stop-color="#000" />
        </linearGradient>
      </defs>
      <rect width="120" height="120" rx="27" fill="url(#mark-bg)" />
      <g fill="none" stroke-linecap="round" stroke-width="9" transform="rotate(-90 60 60)">
        <circle cx="60" cy="60" r="38" stroke="#34c759" stroke-opacity="0.22" />
        <circle cx="60" cy="60" r="38" stroke="#34c759" stroke-dasharray="207 239" />
        <circle cx="60" cy="60" r="26" stroke="#0a84ff" stroke-opacity="0.22" />
        <circle cx="60" cy="60" r="26" stroke="#0a84ff" stroke-dasharray="123 163" />
        <circle cx="60" cy="60" r="14" stroke="#ff9f0a" stroke-opacity="0.22" />
        <circle cx="60" cy="60" r="14" stroke="#ff9f0a" stroke-dasharray="58 88" />
      </g>
    </svg>
    <h1>Know the computer<br />before you buy it.</h1>
    <p class="lede">
      probe checks the drives, battery and locks of the computer it runs on, scores its condition, and
      tells you what it is good for. Everything happens on this computer. Nothing is sent anywhere.
    </p>
    <div class="actions">
      <button class="primary big" onclick={full ? onscan : onscanlimited} disabled={busy}>Scan this computer</button>
      <button class="plain" onclick={onopen} disabled={busy}>Open a saved scan <Icon name="chevron" size={14} stroke={2.2} /></button>
    </div>
    {#if privilege?.elevated}
      <p class="access faint">Running with administrator rights, so the scan has full access.</p>
    {:else if privilege?.can_elevate}
      <p class="access faint">
        You will be asked for the {privilege.method} so probe can read drive health, serials and
        firmware tables. It only reads; it changes nothing.
        <button class="plain inline" onclick={onscanlimited} disabled={busy}>Scan without administrator rights</button>
      </p>
    {:else if privilege}
      <p class="access faint">Full scans need administrator rights, which this copy of probe cannot request: {privilege.reason}</p>
    {/if}
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </section>

  <section class="facts">
    <div class="fact card">
      <span class="badge green"><Icon name="shield" size={20} /></span>
      <h2>A condition score</h2>
      <p class="muted">
        Drive wear and errors, battery capacity, and whether a company, Apple Account or tracking
        agent still has a hold on it, rolled into one number that never hides a serious problem.
      </p>
    </div>
    <div class="fact card">
      <span class="badge blue"><Icon name="sparkle" size={20} /></span>
      <h2>Advice for your use</h2>
      <p class="muted">
        Tell it what you will do, from schoolwork to video editing or gaming, and see whether this
        computer is up to it, and why.
      </p>
    </div>
    <div class="fact card">
      <span class="badge orange"><Icon name="eye" size={20} /></span>
      <h2>Honest about limits</h2>
      <p class="muted">
        It cannot tell whether a computer is stolen; no public register of serials exists. Swollen
        batteries, liquid damage and hinge wear need your own eyes.
      </p>
    </div>
  </section>

  {#if onwipeguide}
    <section class="card selling">
      <span class="badge green"><Icon name="checklist" size={20} /></span>
      <div>
        <h2>Selling a computer?</h2>
        <p class="muted">
          Scan it, then make a seller's report with a QR code buyers can check on their phone. A step-by-step guide shows
          how to wipe it safely before you hand it over.
        </p>
      </div>
      <button onclick={onwipeguide}>Wipe guide</button>
    </section>
  {/if}

  {#if history}
    <History {history} onopen={onopenstored} ondelete={ondeletestored} />
  {/if}

  {#if team}
    <TeamCard {team} counts={history?.counts} onchange={onteamchange} {onsynced} />
  {/if}
</main>

<style>
  .home {
    max-width: 980px;
    margin: 0 auto;
    padding: 64px 24px 56px;
    display: grid;
    gap: 40px;
  }
  .topbar {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    height: 32px;
    margin: -40px 0 -24px;
  }
  .hero {
    display: grid;
    gap: 16px;
    justify-items: center;
    text-align: center;
  }
  .mark {
    width: 88px;
    height: 88px;
    margin-bottom: 8px;
    filter: drop-shadow(0 8px 18px rgba(0, 0, 0, 0.18));
  }
  h1 {
    font-size: 52px;
    line-height: 1.06;
    font-weight: 700;
    letter-spacing: -0.035em;
  }
  .lede {
    max-width: 58ch;
    font-size: 19px;
    line-height: 1.45;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    align-items: center;
    justify-content: center;
    margin-top: 10px;
  }
  .big {
    padding: 13px 30px;
    font-size: 17px;
    font-weight: 500;
  }
  .actions .plain {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 17px;
  }
  .access {
    font-size: 13px;
    max-width: 64ch;
  }
  .inline {
    font-size: 13px;
    padding: 0 2px;
  }
  .error {
    color: var(--red);
    background: var(--red-bg);
    padding: 10px 14px;
    border-radius: 12px;
    max-width: 64ch;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  .fact {
    padding: 22px 22px 24px;
    display: grid;
    gap: 8px;
    align-content: start;
  }
  .badge {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: #fff;
    margin-bottom: 4px;
  }
  .badge.green {
    background: var(--green-fill);
  }
  .badge.blue {
    background: var(--blue-fill);
  }
  .badge.orange {
    background: var(--amber-fill);
  }
  .selling {
    padding: 20px 22px;
    display: flex;
    gap: 16px;
    align-items: center;
  }
  .selling .badge {
    flex: none;
    margin: 0;
  }
  .selling div {
    flex: 1;
    display: grid;
    gap: 4px;
  }
  .selling h2 {
    font-size: 17px;
    font-weight: 650;
  }
  .selling p {
    font-size: 14px;
  }
  .fact h2 {
    font-size: 17px;
    font-weight: 650;
  }
  .fact p {
    font-size: 14px;
  }
  @media (max-width: 760px) {
    .facts {
      grid-template-columns: 1fr;
    }
    h1 {
      font-size: 38px;
    }
  }
  @media (max-width: 560px) {
    .home {
      padding: 36px 16px;
      gap: 28px;
    }
    h1 {
      font-size: 32px;
    }
    .lede {
      font-size: 17px;
    }
  }
</style>
