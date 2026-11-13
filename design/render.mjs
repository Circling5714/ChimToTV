// Xuất logo từ design/logo.html bằng Chrome headless (cần Google Chrome + Node 22+).
//   node design/render.mjs preview <file.png>   → ảnh xem trước cả 3 phương án
//   node design/render.mjs export <a|b|c>       → ghi PNG vào app/src/main/res (+ design/logo-512.png, banner-1280.png)
//   node design/render.mjs ios <a|b|c>          → icon 1024 + logo cho bản iPad (ios/ConBangTV/Assets.xcassets)
//   node design/render.mjs mac <a|b|c>          → các cỡ icon macOS (16…1024) cho bản Mac, cùng bộ AppIcon
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const res = resolve(here, '../app/src/main/res');
const [mode = 'preview', arg] = process.argv.slice(2);
const chrome = process.env.CHROME || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const port = 9340;
const profile = mkdtempSync(join(tmpdir(), 'chimto-logo-'));
const proc = spawn(chrome, ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
  '--no-first-run', '--hide-scrollbars', 'about:blank'], { stdio: 'ignore' });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let targets;
for (let i = 0; i < 40 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); } catch { await sleep(250); }
}
const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
ws.addEventListener('message', ev => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
});
const send = (method, params = {}) => new Promise(r => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const evaluate = async expr => (await send('Runtime.evaluate', { expression: expr, returnByValue: true })).result.result.value;

await send('Page.enable');
await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } });
const query = mode === 'export' || mode === 'ios' || mode === 'mac' ? `?mode=${mode}&concept=${arg || 'a'}` : '?mode=preview';
await send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: pathToFileURL(join(here, 'logo.html')).href + query });
for (let i = 0; i < 100 && (await evaluate('document.title')) !== 'ready'; i++) await sleep(200);
await sleep(300);

const shot = async clip => Buffer.from((await send('Page.captureScreenshot', { format: 'png', clip: { ...clip, scale: 1 }, captureBeyondViewport: true })).result.data, 'base64');

if (mode === 'export' || mode === 'ios' || mode === 'mac') {
  const items = await evaluate(`[...document.querySelectorAll('.asset')].map(d => { const r = d.firstElementChild.getBoundingClientRect(); return { name: d.dataset.name, x: r.x, y: r.y, w: r.width, h: r.height }; })`);
  const total = Math.ceil(Math.max(...items.map(i => i.y + i.h)));
  await send('Emulation.setDeviceMetricsOverride', { width: 1400, height: total, deviceScaleFactor: 1, mobile: false });
  await sleep(300);
  for (const it of items) {
    const out = resolve(res, it.name);
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, await shot({ x: it.x, y: it.y, width: it.w, height: it.h }));
    console.log('ghi', out, `${it.w}x${it.h}`);
  }
} else {
  const h = Math.ceil(await evaluate('document.body.scrollHeight'));
  await send('Emulation.setDeviceMetricsOverride', { width: 1400, height: h, deviceScaleFactor: 1, mobile: false });
  await sleep(300);
  writeFileSync(arg || 'preview.png', await shot({ x: 0, y: 0, width: 1400, height: h }));
  console.log('ghi', arg || 'preview.png');
}
ws.close();
proc.kill();
process.exit(0);
