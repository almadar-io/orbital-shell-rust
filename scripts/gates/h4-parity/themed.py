"""themed.py <src.lolo> <theme> <out.lolo>: a copy of a behavior whose app header declares `theme: "<theme>"` (replacing any
theme line), composing std's mock page seed as `orb serve` would; its sibling locale catalogs
(`locales/<lang>/<name>.json`) are copied beside the copy so the program validates as the original does."""
import os
import re
import shutil
import sys

src, theme, out = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(src).read()
m = re.match(r'(\s*(?:;;[^\n]*\n\s*)*app\s+[\w-]+\s*\[)(.*?)(\n\])', text, re.S)
if not m:
    sys.exit(f"no app header in {src}")
body = re.sub(r'\n\s*theme\s*:\s*"[^"]*"\s*,?', '', m.group(2))
header = f'{m.group(1)}\n  theme: "{theme}",{body}{m.group(3)}'
rest = text[m.end():]
seed = ''
if 'mock_seed_marks' not in text and 'std-mock-page-seed' not in text:
    seed = '\nuses Seed from "std/behaviors/std-mock-page-seed"\norbital MockData = Seed.orbitals.MockData {}\n'
os.makedirs(os.path.dirname(out), exist_ok=True)
open(out, 'w').write(header + rest + seed)
name = os.path.splitext(os.path.basename(src))[0]
out_name = os.path.splitext(os.path.basename(out))[0]
locales = os.path.join(os.path.dirname(src), 'locales')
if os.path.isdir(locales):
    for lang in os.listdir(locales):
        cat = os.path.join(locales, lang, f'{name}.json')
        if os.path.exists(cat):
            dest = os.path.join(os.path.dirname(out), 'locales', lang)
            os.makedirs(dest, exist_ok=True)
            shutil.copy(cat, os.path.join(dest, f'{out_name}.json'))
