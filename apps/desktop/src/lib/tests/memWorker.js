// One share of the memory test. Fills as much memory as it is given with
// known patterns, then reads every value back. A value that comes back
// different from what was written means a faulty memory cell or module.
//
// It can only test memory the system hands to the app, not every cell on
// the modules, so a clean result lowers risk rather than proving the RAM
// sound. A boot-time tester (MemTest86) covers the rest.

const CHUNK_WORDS = (64 * 1024 * 1024) / 4; // 64 MB per block

let stop = false;

const PATTERNS = [
  { name: 'address', at: (i, seed) => (Math.imul(i, 2654435761) + seed) >>> 0 },
  { name: 'inverse address', at: (i, seed) => ~(Math.imul(i, 2654435761) + seed) >>> 0 },
  { name: 'checkerboard', at: (i) => (i & 1 ? 0x55555555 : 0xaaaaaaaa) >>> 0 },
  { name: 'inverse checkerboard', at: (i) => (i & 1 ? 0xaaaaaaaa : 0x55555555) >>> 0 },
];

function allocate(mb) {
  const blocks = [];
  const wanted = Math.floor(mb / 64);
  for (let n = 0; n < wanted; n++) {
    try {
      blocks.push(new Uint32Array(CHUNK_WORDS));
    } catch {
      break; // the system will not give more: test what was granted
    }
  }
  return blocks;
}

onmessage = (e) => {
  if (e.data === 'stop') {
    stop = true;
    return;
  }
  const { mb, seed } = e.data;
  const blocks = allocate(mb);
  const total = blocks.length * PATTERNS.length * 2;
  let done = 0;
  let errors = 0;
  const examples = [];
  postMessage({ type: 'granted', mb: blocks.length * 64 });

  for (const p of PATTERNS) {
    // Write everything first, so values sit in memory while the rest is
    // written, then read it all back.
    for (let b = 0; b < blocks.length; b++) {
      if (stop) return postMessage({ type: 'stopped', errors, examples });
      const block = blocks[b];
      const base = b * CHUNK_WORDS;
      for (let i = 0; i < block.length; i++) block[i] = p.at(base + i, seed);
      postMessage({ type: 'progress', done: ++done, total, errors });
    }
    for (let b = 0; b < blocks.length; b++) {
      if (stop) return postMessage({ type: 'stopped', errors, examples });
      const block = blocks[b];
      const base = b * CHUNK_WORDS;
      for (let i = 0; i < block.length; i++) {
        const want = p.at(base + i, seed);
        if (block[i] !== want) {
          errors++;
          if (examples.length < 5) {
            examples.push({ pattern: p.name, wrote: want.toString(16), read: block[i].toString(16) });
          }
        }
      }
      postMessage({ type: 'progress', done: ++done, total, errors });
    }
  }
  postMessage({ type: 'done', errors, examples, mb: blocks.length * 64 });
};
