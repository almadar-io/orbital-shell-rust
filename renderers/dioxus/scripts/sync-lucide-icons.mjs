import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const version = '0.570.0';
const expectedSha256 = 'ff46553f4bc1d2575aa3c1806a14427a9cbdcdb92df7e85c30b2f3cb178f749c';
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const aliasSource = join(root, '../../../orbital-shell-typescript/renderers/web/src/core/lib/lucideIcons.tsx');
const temporary = mkdtempSync(join(tmpdir(), 'almadar-lucide-'));

try {
  const archive = join(temporary, 'lucide-static.tgz');
  execFileSync('curl', ['--fail', '--location', '--silent', '--show-error', '--output', archive,
    `https://registry.npmjs.org/lucide-static/-/lucide-static-${version}.tgz`]);
  const bytes = readFileSync(archive);
  const actualSha256 = createHash('sha256').update(bytes).digest('hex');
  if (actualSha256 !== expectedSha256) throw new Error(`Lucide archive checksum mismatch: ${actualSha256}`);
  execFileSync('tar', ['-xzf', archive, '-C', temporary,
    'package/package.json', 'package/LICENSE', 'package/icon-nodes.json',
    'package/dist/esm/lucide-static.js']);
  const packageRoot = join(temporary, 'package');
  const metadata = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8'));
  if (metadata.name !== 'lucide-static' || metadata.version !== version) {
    throw new Error('Lucide package name/version mismatch');
  }
  const nodes = JSON.parse(readFileSync(join(packageRoot, 'icon-nodes.json'), 'utf8'));
  const tags = new Set(['path', 'circle', 'rect', 'polyline', 'line', 'polygon', 'ellipse']);
  const attributes = new Set(['cx', 'cy', 'd', 'fill', 'height', 'points', 'r', 'rx', 'ry',
    'width', 'x', 'x1', 'x2', 'y', 'y1', 'y2']);
  for (const [name, shapes] of Object.entries(nodes)) {
    for (const [tag, values] of shapes) {
      if (!tags.has(tag)) throw new Error(`Unsupported Lucide SVG element: ${name}/${tag}`);
      for (const attribute of Object.keys(values)) {
        if (!attributes.has(attribute)) throw new Error(`Unsupported Lucide SVG attribute: ${name}/${attribute}`);
      }
    }
  }
  const exports = readFileSync(join(packageRoot, 'dist/esm/lucide-static.js'), 'utf8');
  const canonicalByExport = new Map();
  const aliases = {};
  for (const line of exports.split('\n')) {
    const match = /^export \{ (.+) \} from '\.\/icons\/([^']+)\.js';$/.exec(line);
    if (!match) continue;
    for (const [, identifier] of match[1].matchAll(/default as ([A-Za-z0-9]+)/g)) {
      canonicalByExport.set(identifier, match[2]);
      const alias = identifier.replace(/([A-Z])([A-Z][a-z])/g, '$1-$2').replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
      if (alias !== match[2] && !nodes[alias]) aliases[alias] = match[2];
    }
  }
  const webAliases = readFileSync(aliasSource, 'utf8');
  const aliasBlock = /const ALIASES: Record<string, LucideIcon> = \{([\s\S]*?)\n\};/.exec(webAliases)?.[1];
  if (!aliasBlock) throw new Error('Could not locate canonical web Lucide aliases');
  for (const line of aliasBlock.split('\n').filter((line) => line.trim())) {
    const match = /^\s*(?:'([^']+)'|([A-Za-z][A-Za-z0-9-]*)):\s*([A-Za-z][A-Za-z0-9]*),?\s*$/.exec(line);
    if (!match) throw new Error(`Unrecognized web Lucide alias: ${line}`);
    const name = match[1] ?? match[2];
    const canonical = canonicalByExport.get(match[3]);
    if (!canonical || !nodes[canonical]) throw new Error(`Unknown Lucide alias target: ${line}`);
    aliases[name] = canonical;
  }
  writeFileSync(join(root, 'assets/lucide-nodes.json'), JSON.stringify(nodes));
  writeFileSync(join(root, 'assets/lucide-aliases.json'), JSON.stringify(aliases));
  writeFileSync(join(root, 'assets/LUCIDE-LICENSE'), readFileSync(join(packageRoot, 'LICENSE')));
  writeFileSync(join(root, 'assets/LUCIDE-SOURCE'),
    `lucide-static ${version}\nhttps://registry.npmjs.org/lucide-static/-/lucide-static-${version}.tgz\nsha256 ${expectedSha256}\nweb-alias-source orbital-shell-typescript/renderers/web/src/core/lib/lucideIcons.tsx\nweb-alias-sha256 ${createHash('sha256').update(webAliases).digest('hex')}\n`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
