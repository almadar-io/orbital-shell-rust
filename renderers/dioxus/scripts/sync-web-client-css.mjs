import { createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';

const base = process.argv[2];
if (!base) throw new Error('usage: node scripts/sync-web-client-css.mjs <orb-serve-url>');
const origin = new URL(base);
const page = await fetch(origin);
if (!page.ok) throw new Error(`${origin}: ${page.status}`);
const html = await page.text();
const hrefs = [...html.matchAll(/<link\b[^>]*rel="stylesheet"[^>]*href="([^"]+)"[^>]*>/g)].map((match) => match[1]);
if (hrefs.length !== 1) throw new Error(`expected one built client stylesheet, found ${hrefs.length}`);
const stylesheet = new URL(hrefs[0], origin);
const response = await fetch(stylesheet);
if (!response.ok) throw new Error(`${stylesheet}: ${response.status}`);
const css = await response.text();
for (const selector of ['.surface-content', '.line-clamp-2', '.rounded-interactive']) {
  if (!css.includes(selector)) throw new Error(`${stylesheet} lacks ${selector}`);
}
const digest = createHash('sha256').update(css).digest('hex');
const output = new URL('../assets/css/web-client.css', import.meta.url);
await writeFile(output, `/* Built web client CSS from ${stylesheet.pathname}; sha256 ${digest}. Regenerate with sync-web-client-css.mjs. */\n${css}\n`);
process.stdout.write(`${output.pathname}\n`);
