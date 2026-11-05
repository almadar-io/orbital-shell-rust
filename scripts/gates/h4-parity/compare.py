"""compare.py <outdir> <baseline.tsv> <baseline.skip> [--record]
Reads <outdir>/run.log. Fails on a capture failure for a behavior not in the skip file, an incomplete run, and a pair whose
delta exceeds its baseline row + H4_SLACK (default 0.5). --record writes the baseline from the run instead."""
import os
import re
import sys

out, base_path, skip_path = sys.argv[1:4]
record = '--record' in sys.argv
slack = float(os.environ.get('H4_SLACK', '0.5'))

skips = {}
if os.path.exists(skip_path):
    for line in open(skip_path):
        line = line.strip()
        if line and not line.startswith('#'):
            name, _, reason = line.partition(' ')
            skips[name] = reason.strip()
deltas, fails, jobs = {}, {}, None
for line in open(os.path.join(out, 'run.log')):
    line = line.strip()
    m = re.match(r'(.+)--(.+)\.pair\.png delta=([\d.]+)', line)
    if m:
        deltas[(m.group(1), m.group(2))] = float(m.group(3))
        continue
    m = re.match(r'(\w+_FAIL) (\S+) (\S+)', line)
    if m:
        fails[(m.group(2), m.group(3))] = m.group(1)
    m = re.match(r'jobs=(\d+)', line)
    if m:
        jobs = int(m.group(1))

if record:
    rows = [(k, d) for k, d in sorted(deltas.items()) if k not in fails and k[0] not in skips]
    with open(base_path, 'w') as f:
        for (b, t), d in rows:
            f.write(f'{b}\t{t}\t{d:.1f}\n')
    print(f'h4: recorded {len(rows)} rows to {base_path}')
    sys.exit(0)

base = {}
if os.path.exists(base_path):
    for line in open(base_path):
        p = line.rstrip('\n').split('\t')
        if len(p) == 3:
            base[(p[0], p[1])] = float(p[2])
problems = []
for (b, t), kind in sorted(fails.items()):
    if b not in skips:
        problems.append(f'capture failure {kind} {b} {t}')
if jobs is not None and len(deltas) + len(fails) < jobs and not any(k[0] not in skips for k in fails):
    problems.append(f'{jobs - len(deltas)} of {jobs} pairs produced no delta line (run incomplete; rerun to resume)')
for (b, t), d in sorted(deltas.items()):
    if b in skips or (b, t) in fails:
        continue
    if (b, t) not in base:
        problems.append(f'no baseline row for {b} {t} (delta {d:.1f}); record one')
    elif d > base[(b, t)] + slack:
        problems.append(f'regression {b} {t}: delta {d:.1f} > baseline {base[(b, t)]:.1f} + {slack}')
improved = sum(1 for k, d in deltas.items() if k in base and d < base[k] - slack)
print(f'h4: pairs {len(deltas)}  capture failures {len(fails)}  skipped behaviors {len(skips)}  improved {improved}')
for p in problems:
    print('  FAIL', p)
if improved:
    print('  note: lower the improved baseline rows (h4.sh <tier> --record) in the commit that fixed them')
sys.exit(1 if problems else 0)
