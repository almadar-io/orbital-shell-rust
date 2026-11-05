# H4 parity gate: web and native desktop paint the same program

`h4.sh <tier> [workers] [outdir]` runs every std behavior of a tier, under every theme in `themes.txt`, through two
real hosts that share one running server. `<tier>` is `core-atoms`, `core-molecules`, `core-organisms`, `learning`
(std folders `ui/core/atoms`, `ui/core/molecules`, `ui/core/organisms` + `ui/core/templates`, `ui/learning/atoms`),
or a path to a list file of behavior paths relative to `packages/almadar-std/behaviors/lolo`.

1. `themed.py` writes a copy of the behavior whose app header declares `theme: "<key>"` (a `.lolo` line, nothing
   implicit), composes std's mock page seed as `orb serve` does, and copies the locale catalogs beside it. `orb resolve`
   lowers it.
2. `orb serve` runs it. The web client is captured with headless Chrome over the DevTools protocol once its DOM has
   been still for 1.5 s (`cdp_shot.mjs`; writes `.web.png` and `.web.html`). Each worker reuses one Chrome.
3. `orb-desktop request` captures the same route against the same server (`Screenshot { route, server }`, the client
   projection of the program, host frames run until quiet), writing `.desktop.png` and `.desktop.tree.json`.
4. `composite.py` writes a side-by-side `.pair.png` and the mean pixel delta.

Then `compare.py` checks the run against `baseline/<tier>.tsv` (`behavior<TAB>theme<TAB>delta`). It exits 1 on:
- a capture failure (`RESOLVE_FAIL`, `SERVE_FAIL`, `DESKTOP_FAIL`) for a behavior not listed in `baseline/<tier>.skip`
  (one behavior per line, then a reason: behaviors with no client UI);
- a pair with no baseline row, or a delta above its baseline row + 0.5 (`H4_SLACK` overrides);
- an incomplete run.

`h4.sh <tier> --record` writes the baseline from the run instead. A fix lowers its rows in the same commit.
Pairs are accepted by looking at them; the delta only ratchets regressions.

Runs are resumable: a finished `.pair.png` is skipped. Use a fresh outdir (default `$TMPDIR/h4-parity-<tier>`) for a
clean verdict, since `run.log` is appended. `progress.sh <outdir> [step]` follows a running tier; `summary.py <outdir>`
prints the worst delta per behavior.

## Requirements

- `orb` (`$ORB`, else on PATH) and `orb-desktop` (`$ORB_DESKTOP`, else beside the resolved `orb`), built with the web
  client. `$ORB` may carry a prefix; `$ORB_GUARD` (when set) is prefixed to every orb run.
- Google Chrome (`$CHROME`, default the macOS app path), Node 22 or newer (built-in WebSocket), zsh, curl, lsof.
- Python 3 that imports Pillow (`$PARITY_PY`, else `python3`); the gate stops with a clear message otherwise.
- Monorepo root found from the script location, or `$ALMADAR_ROOT`. Ports start at `$H4_PORT_BASE` (4911) and
  `$H4_CDP_BASE` (9300), one pair per worker.
- Not part of `scripts/gates/run.sh all`: it needs Chrome and a GPU path for the desktop host. Run `run.sh h4`.
