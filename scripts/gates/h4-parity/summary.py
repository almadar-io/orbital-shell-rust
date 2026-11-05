"""summary.py <outdir> [threshold]: per behavior, the worst theme delta; failures; behaviors above threshold."""
import collections
import os
import re
import sys

out = sys.argv[1]
thr = float(sys.argv[2]) if len(sys.argv) > 2 else 3.0
deltas = collections.defaultdict(dict)
fails = collections.defaultdict(list)
for line in open(os.path.join(out, 'run.log')):
    m = re.match(r'(.+)--(.+)\.pair\.png delta=([\d.]+)', line.strip())
    if m:
        deltas[m.group(1)][m.group(2)] = float(m.group(3))
        continue
    m = re.match(r'(\w+_FAIL) (\S+) (\S+)', line.strip())
    if m:
        fails[m.group(2)].append(f'{m.group(1)}:{m.group(3)}')
web_status = collections.defaultdict(list)
for f in os.listdir(os.path.join(out, 'log')):
    if f.endswith('.chrome.log'):
        text = open(os.path.join(out, 'log', f)).read()
        if not text.startswith('WEB_OK'):
            name, theme = f[:-len('.chrome.log')].rsplit('--', 1)
            web_status[name].append(f'{theme}:{text.split()[0] if text.split() else "EMPTY"}')
names = sorted(set(deltas) | set(fails))
over = [(max(deltas[n].values()) if deltas[n] else 999, n) for n in names]
over.sort(reverse=True)
print(f'behaviors {len(names)}  pairs {sum(len(v) for v in deltas.values())}')
print('== failures')
for n in sorted(fails):
    print(n, ' '.join(sorted(set(fails[n]))))
print('== web not settled')
for n in sorted(web_status):
    print(n, ' '.join(web_status[n]))
print(f'== worst delta per behavior (>{thr})')
for d, n in over:
    if d > thr:
        worst = max(deltas[n], key=deltas[n].get) if deltas[n] else '-'
        print(f'{d:6.1f} {n} (worst {worst})')
print('== clean (<= threshold on every theme)')
print(' '.join(n for d, n in sorted(over) if d <= thr))
