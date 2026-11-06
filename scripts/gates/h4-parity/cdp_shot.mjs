// cdp_shot.mjs <url> <out.png> <profile-dir> <debug-port>
// Capture <url> at 1280x900 once the page has rendered app content, no request is in flight (the open-ended EventSource
// stream excepted), and its DOM has been
// unchanged for 1.5 s since the last DOM change or request completion (25 s cap).
// With CDP_REUSE=1 it opens a tab in the Chrome already listening on <debug-port> (one per worker) and closes it after;
// otherwise it launches its own headless Chrome. Writes <out>.png and <out>.html; prints WEB_OK / WEB_TIMEOUT.
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const [url, out, profile, port] = process.argv.slice(2);
const reuse = process.env.CDP_REUSE === '1';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chrome = reuse ? null : spawn(process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', [
  '--headless=new', '--hide-scrollbars', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, '--disable-site-isolation-trials',
  '--window-size=1280,900', '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });

async function devtools(path, method = 'GET') {
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}${path}`, { method });
      if (r.ok) return await r.json();
    } catch { /* starting */ }
    await sleep(100);
  }
  throw new Error(`chrome devtools did not answer ${path}`);
}

let exitCode = 0;
let targetId;
try {
  let wsUrl;
  if (reuse) {
    const target = await devtools('/json/new?about:blank', 'PUT');
    targetId = target.id;
    wsUrl = target.webSocketDebuggerUrl;
  } else {
    for (let i = 0; i < 100 && !wsUrl; i++) {
      const list = await devtools('/json/list');
      wsUrl = list.find((t) => t.type === 'page')?.webSocketDebuggerUrl;
      if (!wsUrl) await sleep(100);
    }
  }
  const ws = new WebSocket(wsUrl);
  await new Promise((r) => ws.addEventListener('open', r, { once: true }));
  let id = 0;
  const pending = new Map();
  const errors = [];
  const inFlight = new Map();
  let netChangedAt = Date.now();
  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    if (msg.method === 'Network.requestWillBeSent' && msg.params.type !== 'EventSource') { inFlight.set(msg.params.requestId, `${msg.params.type ?? ''} ${msg.params.request.url}`); netChangedAt = Date.now(); }
    if (msg.method === 'Network.loadingFinished' || msg.method === 'Network.loadingFailed') { inFlight.delete(msg.params.requestId); netChangedAt = Date.now(); }
    if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails?.exception?.description ?? msg.params.exceptionDetails?.text);
    if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') errors.push(msg.params.args.map((a) => a.value ?? a.description).join(' '));
  });
  const send = (method, params = {}) => new Promise((r) => { const n = ++id; pending.set(n, r); ws.send(JSON.stringify({ id: n, method, params })); });
  await send('Runtime.enable');
  await send('Page.enable');
  await send('Network.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  const probe = `(() => { const root = document.getElementById('root') ?? document.body; return JSON.stringify({ n: root.innerHTML.length, text: root.innerText.length, theme: document.documentElement.getAttribute('data-theme') ?? document.querySelector('[data-theme]')?.getAttribute('data-theme') ?? '' }); })()`;
  const start = Date.now();
  let last = '', stableSince = Date.now(), status = 'WEB_TIMEOUT';
  while (Date.now() - start < 25000) {
    await sleep(250);
    const r = await send('Runtime.evaluate', { expression: probe, returnByValue: true });
    const v = r.result?.result?.value ?? '';
    if (v !== last) { last = v; stableSince = Date.now(); continue; }
    const parsed = JSON.parse(v || '{"n":0}');
    if (parsed.n > 0 && inFlight.size === 0 && Date.now() - Math.max(stableSince, netChangedAt) >= 1500) { status = 'WEB_OK'; break; }
  }
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
  const dom = await send('Runtime.evaluate', { expression: 'document.documentElement.outerHTML', returnByValue: true });
  writeFileSync(out.replace(/\.png$/, '.html'), dom.result?.result?.value ?? '');
  console.log(status, last);
  for (const r of inFlight.values()) console.log('IN_FLIGHT', r);
  for (const e of errors) console.log('CONSOLE_ERROR', e);
  ws.close();
} catch (e) {
  console.log('WEB_FAIL', e?.message ?? String(e));
  exitCode = 1;
} finally {
  if (reuse && targetId) await fetch(`http://127.0.0.1:${port}/json/close/${targetId}`).catch(() => {});
  if (chrome) chrome.kill('SIGKILL');
}
process.exit(exitCode);
