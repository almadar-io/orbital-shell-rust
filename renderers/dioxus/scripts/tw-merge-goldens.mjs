import { createRequire } from 'node:module';
import { readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(here, '../../../../orbital-shell-typescript/renderers/web');
const webSrc = path.join(webRoot, 'src');
const require = createRequire(path.join(webRoot, 'package.json'));
const { extendTailwindMerge } = require('tailwind-merge');
const { clsx } = require('clsx');

const themeScaleSource = readFileSync(path.join(webSrc, 'core/lib/theme-scale.ts'), 'utf8');
const KEYS = new Function(`${themeScaleSource.replace(/export const/, 'const').replace(/ as const;/, ';')}; return THEME_SCALE_KEYS;`)();
const scale = (keys) => [...keys];
const twMerge = extendTailwindMerge({
  extend: {
    theme: { spacing: scale(KEYS.spacing), borderRadius: scale(KEYS.borderRadius) },
    classGroups: {
      h: [{ h: scale(KEYS.height) }],
      'min-h': [{ 'min-h': scale(KEYS.minHeight) }],
      w: [{ w: scale(KEYS.width) }],
      'font-size': [{ text: scale(KEYS.fontSize) }],
      shadow: [{ shadow: scale(KEYS.boxShadow) }],
    },
  },
});
const cn = (...inputs) => twMerge(clsx(inputs));

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (full.endsWith('.tsx')) out.push(full);
  }
  return out;
}

const classToken = /^[A-Za-z0-9:[\]/!_.@&>*%#,()=+-]+$/;
const literals = new Set();
for (const file of [...walk(path.join(webSrc, 'components')), ...walk(path.join(webSrc, 'core'))]) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(/(['"`])([^'"`\n$]{6,200}?)\1/g)) {
    const tokens = m[2].trim().split(/\s+/);
    if (tokens.length >= 2 && tokens.every((t) => classToken.test(t) && /[a-z]-|^[a-z]+$|:/.test(t))) literals.add(m[2].trim());
  }
}
const bases = [...literals].sort();

let seed = 20260505;
const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
const pick = (arr) => arr[Math.floor(rand() * arr.length)];

const overrides = [
  'p-2', 'px-4', 'py-1', 'p-card-md', 'pt-section', 'm-0', 'mx-auto', 'mt-4', '-mt-2', 'my-2',
  'h-auto', 'h-full', 'h-button-lg', 'h-icon-default', 'h-[calc(100vh-4rem)]', 'h-screen', 'min-h-0', 'min-h-row-compact', 'min-h-[200px]',
  'w-full', 'w-icon-default', 'w-1/2', 'w-[50%]', 'w-px', 'max-w-md', 'size-4',
  'text-sm', 'text-display-1', 'text-display-2', 'text-[13px]', 'text-xs', 'text-center', 'text-red-500', 'text-primary', 'text-red-500/50', 'text-[#fff]',
  'bg-red-500', 'bg-primary/20', 'bg-[#123456]', 'bg-transparent', 'border', 'border-2', 'border-red-500', 'border-t', 'border-x-0',
  'rounded-lg', 'rounded-none', 'rounded-container', 'rounded-interactive', 'rounded-pill', 'rounded-full', 'rounded-t-md',
  'shadow-lg', 'shadow-none', 'shadow-elevation-card', 'shadow-elevation-dialog', 'shadow-[0_0_4px_red]',
  'flex', 'hidden', 'block', 'inline-flex', 'grid', 'flex-col', 'flex-row', 'flex-1', 'flex-wrap', 'items-center', 'justify-between', 'gap-2', 'gap-x-4', 'gap-card-sm',
  'font-bold', 'font-normal', 'leading-tight', 'tracking-wide', 'truncate', 'overflow-hidden', 'overflow-y-auto', 'opacity-50', 'z-10', 'relative', 'absolute', 'inset-0', 'top-0',
  'cursor-pointer', 'transition-all', 'duration-200', 'ring-2', 'ring-primary', 'outline-none', 'underline', 'uppercase',
  'hover:bg-red-500', 'hover:p-4', 'focus:ring-2', 'md:flex', 'md:p-4', 'sm:hidden', 'lg:w-1/3', 'dark:bg-black', '@sm:flex', '@md:p-2', '@container',
  'md:hover:bg-blue-500', 'group-hover:opacity-100', '[&>*]:p-2', '[&>*]:p-4', '!p-2', '!hidden', 'md:!p-4', 'p-[3px]', 'w-[--sidebar]', '[mask-type:alpha]', '[mask-type:luminance]',
  'custom-class', 'surface-content',
];

const inputs = new Set();
const add = (s) => inputs.add(s.trim().replace(/\s+/g, ' '));

const sample = (arr, n) => arr.filter((_, i) => i % Math.max(1, Math.floor(arr.length / n)) === 0).slice(0, n);
for (const b of sample(bases, 100)) add(b);

const stems = ['px-4 py-1', 'p-2', 'h-button-md', 'min-h-input-md', 'text-sm', 'text-display-1', 'rounded-interactive', 'shadow-elevation-card', 'flex flex-col gap-2', 'w-icon-default h-icon-default', 'border border-red-500 bg-red-500/10', 'hover:bg-red-500 md:flex'];
for (const s of stems) for (const o of sample(overrides, 12)) add(`${s} ${o}`);

for (let i = 0; i < 250; i += 1) {
  const base = pick(bases);
  const count = 1 + Math.floor(rand() * 3);
  const extra = Array.from({ length: count }, () => pick(overrides)).join(' ');
  add(`${base} ${extra}`);
}

const handwritten = [
  '', 'p-2', '  p-2   p-4  ', 'px-2 px-4', 'p-4 px-2', 'px-2 p-4', 'inset-x-1 inset-1', 'inset-1 inset-x-2', '-inset-1 inset-2',
  'text-lg text-display-1', 'text-display-1 text-lg', 'text-red-500 text-sm text-blue-400', 'text-[length:var(--x)] text-red-500', 'text-[color:var(--x)] text-lg',
  'bg-red-500 bg-[url(foo.png)]', 'bg-[length:10px] bg-cover', 'border-[3px] border-red-500', 'border-[color:red] border-2',
  'shadow-[inset_0_1px_2px_black] shadow-md', 'shadow-md shadow-red-500', 'shadow-elevation-card shadow-lg',
  'rounded-s-lg rounded-l-md', 'rounded-lg rounded-tl-none', 'rounded-container rounded-pill',
  'w-1/2 w-3/4', 'w-icon-default w-4', 'size-4 w-6', 'w-6 size-4', 'h-button-sm min-h-button-sm h-auto',
  'leading-4 text-sm', 'text-sm leading-4', 'font-sans font-bold', 'font-bold font-[600]', 'font-[600] font-bold',
  'hover:p-2 p-4 focus:p-2', 'p-4 hover:p-2 hover:p-3', 'md:hover:p-2 hover:md:p-3', '[&>*]:p-2 [&>*]:p-3 p-1', '[&_p]:mt-2 [&_p]:mt-4',
  'group-hover:flex group-hover:hidden', 'data-[state=open]:p-2 data-[state=open]:p-4 data-[state=closed]:p-1',
  '!p-2 !p-4', '!p-2 p-4 p-6', 'p-2! p-4', 'hover:!p-2 hover:p-4',
  'w-1/2', 'w-[calc(100%-2rem)] w-full', 'translate-x-1/2 -translate-x-1/2', '-translate-x-1/2 translate-x-4',
  'bg-red-500/50 bg-red-600', 'bg-red-500/50 bg-red-600/20', 'text-white/80 text-black',
  'flex grid', 'hidden flex', 'block inline-block', 'flex-1 flex-none', 'flex-col flex-row', 'flex-wrap flex-nowrap',
  'items-start items-center', 'justify-start justify-end', 'self-auto self-center', 'gap-1 gap-x-2 gap-y-3 gap-4', 'gap-x-2 gap-4',
  'space-x-2 space-x-4', 'space-y-1 space-y-2',
  'overflow-hidden overflow-x-auto', 'overflow-x-auto overflow-hidden', 'overflow-auto overflow-y-hidden',
  'truncate text-ellipsis', 'text-ellipsis truncate', 'line-clamp-2 line-clamp-3',
  'transition transition-colors', 'duration-100 duration-300', 'ease-in ease-out',
  'opacity-0 opacity-100', 'z-10 z-50', 'z-[999] z-10',
  'grid-cols-2 grid-cols-3', 'col-span-2 col-span-full', 'row-span-2 row-start-1',
  'mx-auto mr-2', 'mr-2 mx-auto', 'm-1 mx-2 mt-3', '-m-1 m-2',
  'ring-1 ring-2 ring-offset-2', 'ring-red-500 ring-offset-red-200', 'outline outline-2 outline-red-500',
  'underline line-through', 'uppercase lowercase', 'italic not-italic',
  'sr-only not-sr-only', 'pointer-events-none pointer-events-auto', 'select-none select-text',
  'cursor-pointer cursor-not-allowed', 'animate-spin animate-pulse',
  'bg-gradient-to-r from-red-500 to-blue-500 bg-none', 'from-red-500 from-10%', 'via-red-500 via-blue-500',
  '[mask-type:alpha] [mask-type:luminance]', '[--foo:1px] [--foo:2px]', '[--foo:1px] [--bar:2px]',
  'p-px p-0.5 p-1.5', 'p-card-sm p-card-md', 'px-dialog px-4', 'pt-section pt-2', 'm-section m-2',
  'text-display-2 text-display-1 text-xl', 'text-base text-display-1',
  'h-icon-default h-4', 'h-4 h-icon-default', 'min-h-row-compact min-h-0', 'min-h-0 min-h-row-spacious',
  'h-row-normal h-full', 'h-[10px] h-button-lg',
  'font-[family-name:var(--f)] font-bold', 'tabular-nums slashed-zero', 'normal-nums oldstyle-nums',
  'sm:p-2 md:p-4 lg:p-6 xl:p-8 p-1', '@sm:p-2 @sm:p-4', '@container/main @container',
  'aspect-square aspect-video', 'object-cover object-contain', 'object-center object-top',
  'break-words break-all', 'whitespace-nowrap whitespace-normal',
  'w-[10px] h-[10px] size-[20px]', 'w-[20px] size-[10px]', 'size-[10px] w-[20px]',
  'divide-x divide-x-2', 'divide-red-500 divide-blue-500', 'content-[""] content-["x"]',
  'fill-red-500 fill-current', 'stroke-2 stroke-red-500', 'stroke-red-500 stroke-[3px]',
  'backdrop-blur-sm backdrop-blur', 'blur-sm blur-lg', 'brightness-50 brightness-[0.3]',
  'scroll-m-2 scroll-mx-4', 'snap-x snap-y', 'touch-none touch-pan-x',
  'order-1 order-last', 'basis-1/2 basis-full', 'grow grow-0', 'shrink shrink-0',
  'top-0 inset-0', 'inset-0 top-0', 'start-0 left-0 inset-x-0',
  'will-change-auto will-change-transform', 'bg-clip-text bg-clip-border', 'bg-origin-border bg-origin-padding',
  'table-auto table-fixed', 'border-collapse border-separate', 'float-left float-right', 'clear-both clear-none',
  'isolate isolation-auto', 'mix-blend-multiply mix-blend-screen', 'resize-none resize',
  'list-disc list-decimal', 'appearance-none appearance-auto', 'caret-red-500 caret-blue-500', 'accent-red-500 accent-auto',
  'hover:text-red-500 hover:text-blue-500 text-green-500', 'focus-visible:ring-2 focus-visible:ring-4', 'disabled:opacity-50 disabled:opacity-100',
  'aria-[sort=ascending]:bg-red-500 aria-[sort=ascending]:bg-blue-500', 'supports-[display:grid]:grid supports-[display:grid]:flex',
  'min-w-0 min-w-full', 'max-w-sm max-w-[10px]', 'max-h-screen max-h-full', 'min-h-screen h-screen',
];
for (const h of handwritten) add(h);

const list = [...inputs].map((input) => ({ input, output: cn(input) }));
const outDir = path.resolve(here, '../tests/fixtures');
mkdirSync(outDir, { recursive: true });
writeFileSync(path.join(outDir, 'tw_merge_goldens.json'), `${JSON.stringify(list, null, 1)}\n`);
process.stdout.write(`${list.length} goldens (${bases.length} real base strings)\n`);
