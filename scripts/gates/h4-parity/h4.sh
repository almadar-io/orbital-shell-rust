#!/usr/bin/env bash
# H4 gate: web and native desktop paint the same std behavior under every listed theme; ratchet the pixel delta.
# Usage: h4.sh <core-atoms|core-molecules|core-organisms|learning|list-file> [workers] [outdir] [--record]
# Needs Chrome and the orb-desktop GPU path, so run.sh keeps it out of `all`. See README.md.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
record=0; args=()
for a in "$@"; do if [ "$a" = --record ]; then record=1; else args+=("$a"); fi; done
tier=${args[0]:-}; workers=${args[1]:-4}
if [ -z "$tier" ]; then echo "usage: h4.sh <core-atoms|core-molecules|core-organisms|learning|list-file> [workers] [outdir] [--record]"; exit 2; fi
export ROOT=${ALMADAR_ROOT:-$(cd "$HERE/../../../../../.." && pwd)}
STD=$ROOT/packages/almadar-std/behaviors/lolo
PY=${PARITY_PY:-python3}
if ! "$PY" -c 'import PIL' 2>/dev/null; then echo "h4: $PY cannot import PIL (Pillow); install it or set PARITY_PY"; exit 2; fi
case "$tier" in
  core-atoms) dirs="ui/core/atoms" ;;
  core-molecules) dirs="ui/core/molecules" ;;
  core-organisms) dirs="ui/core/organisms ui/core/templates" ;;
  learning) dirs="ui/learning/atoms" ;;
  *) dirs="" ;;
esac
name=$tier
outdir=${args[2]:-}
if [ -z "$dirs" ]; then
  [ -f "$tier" ] || { echo "h4: unknown tier or list file: $tier"; exit 2; }
  name=$(basename "$tier" .txt)
fi
[ -n "$outdir" ] || outdir=${TMPDIR:-/tmp}/h4-parity-$name
mkdir -p "$outdir"
outdir=$(cd "$outdir" && pwd)
if [ -n "$dirs" ]; then
  : > "$outdir/list.txt"
  for d in $dirs; do (cd "$STD" && ls "$d"/*.lolo) >> "$outdir/list.txt"; done
  list=$outdir/list.txt
else
  list=$(cd "$(dirname "$tier")" && pwd)/$(basename "$tier")
fi
[ -s "$list" ] || { echo "h4: empty behavior list for $tier"; exit 2; }
echo "h4: $tier: $(wc -l < "$list" | tr -d ' ') behaviors, workers $workers, out $outdir"
zsh "$HERE/run_tier.sh" "$list" "$outdir" "$workers" || exit 2
base=$HERE/baseline/$name.tsv; skip=$HERE/baseline/$name.skip
if [ $record = 1 ]; then "$PY" "$HERE/compare.py" "$outdir" "$base" "$skip" --record; exit $?; fi
"$PY" "$HERE/summary.py" "$outdir" | sed -n '1,/^== worst/p' | head -40
"$PY" "$HERE/compare.py" "$outdir" "$base" "$skip"
