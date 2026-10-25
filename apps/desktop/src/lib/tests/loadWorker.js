// One processor core's worth of steady work for the load test. Reports how
// many work units it finished in each tenth of a second.
let running = false;

function chunk() {
  const start = performance.now();
  let units = 0;
  let x = 1;
  while (performance.now() - start < 100) {
    for (let i = 1; i < 2000; i++) x = (x * 1.000001 + Math.sqrt(i)) % 1e6;
    units++;
  }
  postMessage({ units, x });
  if (running) setTimeout(chunk, 0);
}

onmessage = (e) => {
  if (e.data === 'start' && !running) {
    running = true;
    chunk();
  } else if (e.data === 'stop') {
    running = false;
  }
};
