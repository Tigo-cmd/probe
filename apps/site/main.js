// probe landing page: scroll reveals, the hero ring, the pixel dissolve,
// rotating statements, and download links that match the visitor's system.
// No dependencies. Everything degrades to a static page without script or
// with reduced motion.

// The one place that names the GitHub repository. Change it here if the
// repository moves (for example into an organisation).
const REPO_PATH = 'Tigo-cmd/probe';
const REPO = `https://github.com/${REPO_PATH}`;
const API = `https://api.github.com/repos/${REPO_PATH}`;
const LATEST = `${REPO}/releases/latest/download/`;
const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

// ------------------------------------------------------------ downloads

/** 'windows' | 'mac' | 'linux' | null, from the browser's platform hints. */
function detectOS() {
  const p = (navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
  const ua = navigator.userAgent.toLowerCase();
  if (/android|iphone|ipad|ipod/.test(ua)) return null;
  if (p.includes('win') || ua.includes('windows')) return 'windows';
  if (p.includes('mac') || ua.includes('mac os')) return 'mac';
  if (p.includes('linux') || ua.includes('linux') || ua.includes('x11')) return 'linux';
  return null;
}

const OS = {
  windows: { label: 'Download for Windows', file: 'probe-windows-x64-setup.exe' },
  mac: { label: 'Download for macOS', file: 'probe-macos-universal.dmg' },
  linux: { label: 'Download for Linux', file: 'probe-linux-amd64.deb' },
};

function setupDownloads() {
  const os = detectOS();
  document.querySelectorAll('[data-file]').forEach((a) => {
    a.href = LATEST + a.dataset.file;
    a.setAttribute('download', '');
  });
  document.querySelectorAll('[data-releases]').forEach((a) => (a.href = `${REPO}/releases`));
  document.querySelectorAll('[data-repo]').forEach((a) => (a.href = REPO));
  document.querySelectorAll('[data-repo-issues]').forEach((a) => (a.href = `${REPO}/issues`));
  checkRelease();
  if (!os) return;
  document.querySelector(`.dl[data-os="${os}"]`)?.classList.add('yours');
  // The main buttons download straight away for the visitor's system;
  // the download section stays one click away for everything else.
  document.querySelectorAll('[data-download-link]').forEach((a) => {
    if (a.classList.contains('nav-cta')) return;
    a.href = LATEST + OS[os].file;
    a.setAttribute('download', '');
  });
  document.querySelectorAll('[data-download-label]').forEach((s) => (s.textContent = OS[os].label));
}

// ---------------------------------------------------------------- stars

/** Shows the repository's star count next to the Star buttons, if GitHub answers. */
async function setupStars() {
  const slots = document.querySelectorAll('[data-stars]');
  if (!slots.length) return;
  try {
    const r = await fetch(API, { headers: { Accept: 'application/vnd.github+json' } });
    if (!r.ok) return;
    const n = (await r.json()).stargazers_count;
    if (typeof n !== 'number') return;
    const text = n >= 1000 ? `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k` : String(n);
    slots.forEach((s) => {
      s.textContent = text;
      s.hidden = false;
    });
  } catch {
    // Offline or rate-limited: the buttons work without a count.
  }
}

// --------------------------------------------------------------- donate

/** Copy buttons on the donate page put the value next to them on the clipboard. */
function setupCopy() {
  document.querySelectorAll('[data-copy]').forEach((b) =>
    b.addEventListener('click', async () => {
      const value = document.getElementById(b.dataset.copy)?.textContent.trim();
      if (!value) return;
      try {
        await navigator.clipboard.writeText(value);
        b.dataset.done = '1';
        b.setAttribute('aria-label', 'Copied');
        setTimeout(() => {
          delete b.dataset.done;
          b.setAttribute('aria-label', 'Copy');
        }, 1600);
      } catch {
        // Clipboard blocked: the value is still selectable by hand.
      }
    }),
  );
}

/**
 * Before the first release exists, every download link would be a GitHub
 * 404. Ask GitHub which files the latest release has, and send any link to a
 * missing file to the download section instead, with a short notice.
 */
async function checkRelease() {
  let names;
  try {
    const r = await fetch(`${API}/releases/latest`, {
      headers: { Accept: 'application/vnd.github+json' },
    });
    if (r.status === 404) names = new Set();
    else if (!r.ok) return; // rate-limited or down: keep the direct links
    else names = new Set(((await r.json()).assets ?? []).map((a) => a.name));
  } catch {
    return;
  }
  let missing = false;
  document.querySelectorAll('a[href^="' + LATEST + '"]').forEach((a) => {
    if (names.has(a.href.slice(LATEST.length))) return;
    missing = true;
    a.href = '#download';
    a.removeAttribute('download');
  });
  if (missing) document.querySelectorAll('[data-dl-pending]').forEach((p) => (p.hidden = false));
}

// ------------------------------------------------------------- reveals

function countUp(el) {
  const target = Number(el.dataset.count);
  const suffix = el.dataset.suffix ?? '';
  if (reduced || target === 0) {
    el.textContent = `${target}${suffix}`;
    return;
  }
  const start = performance.now();
  const dur = 1600;
  const tick = (now) => {
    const t = Math.min(1, (now - start) / dur);
    const eased = 1 - Math.pow(1 - t, 3);
    el.textContent = `${Math.round(target * eased)}${suffix}`;
    if (t < 1) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function setupReveals() {
  const targets = document.querySelectorAll('[data-reveal], [data-stairs], [data-stack], .step, .tile, [data-count]');
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        const el = e.target;
        el.classList.add('in');
        if (el.dataset.count !== undefined && !el.dataset.counted) {
          el.dataset.counted = '1';
          countUp(el);
        }
        io.unobserve(el);
      }
    },
    { threshold: 0.18, rootMargin: '0px 0px -40px 0px' },
  );
  targets.forEach((t) => io.observe(t));
}

// The hero ring sweeps to the example score once the page has loaded.
function setupRing() {
  const arc = document.querySelector('[data-ring]');
  if (!arc) return;
  const value = Number(arc.dataset.ring);
  requestAnimationFrame(() =>
    setTimeout(() => (arc.style.strokeDashoffset = String(100 - value)), reduced ? 0 : 500),
  );
}

// ---------------------------------------------------------- hero grid

/** Lighter squares that fade in and out on the hero grid. */
function setupTwinkle() {
  const grid = document.querySelector('[data-twinkle]');
  if (!grid || reduced) return;
  const cell = 96;
  const place = () => {
    grid.querySelectorAll('i').forEach((i) => i.remove());
    const cols = Math.ceil(grid.clientWidth / cell);
    const rows = Math.ceil((grid.clientHeight * 0.55) / cell);
    const offset = (grid.clientWidth / 2) % cell; // grid is centred
    for (let n = 0; n < Math.min(14, cols * rows / 4); n++) {
      const i = document.createElement('i');
      i.style.left = `${offset + Math.floor(Math.random() * cols) * cell - cell + 1}px`;
      i.style.top = `${Math.floor(Math.random() * rows) * cell + 1}px`;
      i.style.animationDelay = `${(Math.random() * 6).toFixed(2)}s`;
      i.style.animationDuration = `${(5 + Math.random() * 4).toFixed(2)}s`;
      grid.appendChild(i);
    }
  };
  place();
  let t;
  window.addEventListener('resize', () => {
    clearTimeout(t);
    t = setTimeout(place, 250);
  });
}

// --------------------------------------------------------- pixel band

/**
 * Covers the report screenshot with a field of grey, white and coral
 * squares that clear in a sweep when the band scrolls into view.
 */
function setupPixels() {
  const wrap = document.querySelector('[data-pixels]');
  if (!wrap) return;
  const canvas = wrap.querySelector('canvas');
  const img = wrap.querySelector('img');
  if (reduced) {
    canvas.remove();
    return;
  }
  const ctx = canvas.getContext('2d');
  const palette = ['#dcdcdc', '#c7c7c7', '#efefef', '#b9b9b9', '#ffffff', '#c9463a', '#e2e2e2'];
  let cells = [];
  let size = 28;
  let start = null;
  let running = false;

  function build() {
    const r = wrap.getBoundingClientRect();
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    canvas.width = Math.round(r.width * dpr);
    canvas.height = Math.round(r.height * dpr);
    size = Math.max(18, Math.round(r.width / 44)) * dpr;
    cells = [];
    const cols = Math.ceil(canvas.width / size);
    const rows = Math.ceil(canvas.height / size);
    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        // Clear from the top-left in a diagonal sweep, with jitter.
        const sweep = (x / cols) * 0.55 + (y / rows) * 0.45;
        cells.push({
          x: x * size,
          y: y * size,
          c: palette[(Math.random() * (Math.random() < 0.12 ? palette.length : palette.length - 2)) | 0],
          at: sweep * 1300 + Math.random() * 700,
        });
      }
    }
    draw(0);
  }

  function draw(elapsed) {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    let left = 0;
    for (const cell of cells) {
      const fade = 1 - Math.min(1, Math.max(0, (elapsed - cell.at) / 260));
      if (fade <= 0) continue;
      left++;
      ctx.globalAlpha = fade;
      ctx.fillStyle = cell.c;
      ctx.fillRect(cell.x, cell.y, size - 1, size - 1);
    }
    ctx.globalAlpha = 1;
    return left;
  }

  function frame(now) {
    if (start === null) start = now;
    const left = draw(now - start);
    if (left > 0) requestAnimationFrame(frame);
    else running = false;
  }

  const go = () => {
    if (running || start !== null) return;
    running = true;
    requestAnimationFrame(frame);
  };
  if (img.complete) build();
  else img.addEventListener('load', build, { once: true });
  window.addEventListener('resize', () => start === null && build());
  new IntersectionObserver(
    (entries) => entries.some((e) => e.isIntersecting) && go(),
    { threshold: 0.35 },
  ).observe(wrap);
}

// ---------------------------------------------------------- statements

function setupStatements() {
  const quotes = [...document.querySelectorAll('[data-statements] p')];
  const tabs = [...document.querySelectorAll('[data-statement]')];
  if (!quotes.length) return;
  let current = 0;
  let timer;
  const show = (n) => {
    current = n;
    quotes.forEach((q, i) => q.classList.toggle('is-on', i === n));
    tabs.forEach((t, i) => t.setAttribute('aria-selected', String(i === n)));
  };
  const auto = () => {
    clearInterval(timer);
    if (!reduced) timer = setInterval(() => show((current + 1) % quotes.length), 6000);
  };
  tabs.forEach((t) =>
    t.addEventListener('click', () => {
      show(Number(t.dataset.statement));
      auto();
    }),
  );
  auto();
}

// ---------------------------------------------------------------- nav

function setupNav() {
  const nav = document.querySelector('[data-nav]');
  const toggle = document.querySelector('[data-nav-toggle]');
  const onScroll = () => nav.classList.toggle('scrolled', window.scrollY > 20);
  onScroll();
  window.addEventListener('scroll', onScroll, { passive: true });
  toggle?.addEventListener('click', () => {
    const open = nav.classList.toggle('open');
    toggle.setAttribute('aria-expanded', String(open));
  });
  nav.querySelectorAll('.nav-links a').forEach((a) =>
    a.addEventListener('click', () => {
      nav.classList.remove('open');
      toggle?.setAttribute('aria-expanded', 'false');
    }),
  );
}

document.querySelectorAll('[data-year]').forEach((s) => (s.textContent = String(new Date().getFullYear())));
setupNav();
setupDownloads();
setupReveals();
setupRing();
setupTwinkle();
setupPixels();
setupStatements();
setupStars();
setupCopy();
