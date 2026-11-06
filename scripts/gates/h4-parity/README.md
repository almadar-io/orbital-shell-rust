# H4 parity gate: web and native desktop paint the same program

`h4.sh <tier> [workers] [outdir]` runs every std behavior of a tier, under every theme in `themes.txt`, through two
real hosts that share one running server. `<tier>` is `core-atoms`, `core-molecules`, `core-organisms`, `learning`
(std folders `ui/core/atoms`, `ui/core/molecules`, `ui/core/organisms` + `ui/core/templates`, `ui/learning/atoms`),
or a path to a list file of behavior paths relative to `packages/almadar-std/behaviors/lolo`.

1. `themed.py` writes a copy of the behavior whose app header declares `theme: "<key>"` (a `.lolo` line, nothing
   implicit), composes std's mock page seed as `orb serve` does, and copies the locale catalogs beside it. `orb resolve`
   lowers it.
2. `orb serve` runs it. The web client is captured with headless Chrome over the DevTools protocol once no request
   is in flight (the server's open-ended `EventSource` stream excepted) and its DOM has been still for 1.5 s since the
   last change or request completion (`cdp_shot.mjs`; writes `.web.png` and `.web.html`, and logs `WEB_OK`, or
   `WEB_TIMEOUT` with the requests still in flight). Anything but `WEB_OK` is a `WEB_FAIL`: a page that never settled
   is not scored. Each worker reuses one Chrome, run without site isolation so a cross-site iframe's requests
   report to the page's own DevTools session.
3. `orb-desktop request` captures the same route against the same server (`Screenshot { route, server }`, the client
   projection of the program, host frames run until quiet), writing `.desktop.png` and `.desktop.tree.json`.
   A program with no non-protocol route page has nothing a user can open: `pair.sh` prints `NO_ROUTE <name> <theme>`
   and captures nothing; `summary.py` and `compare.py` list those behaviors apart and leave them out of deltas.
4. `composite.py` writes a side-by-side `.pair.png` and the mean pixel delta.

Then `compare.py` checks the run against `baseline/<tier>.tsv` (`behavior<TAB>theme<TAB>delta`). It exits 1 on:
- a capture failure (`RESOLVE_FAIL`, `SERVE_FAIL`, `DESKTOP_FAIL`, `WEB_FAIL`) for a behavior not listed in `baseline/<tier>.skip`
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

## Running from a worktree

orb resolves behavior packages and the language data (`@almadar/core`) by walking up from its working directory, and a
root without its own `node_modules/@almadar/core` falls back to the main checkout's. In a worktree, link the worktree's
core at its root (`node_modules/@almadar/core -> orbital-rust/crates/orbital-shell-typescript/core`) and set
`ALMADAR_ROOT` to the worktree, or both legs silently read the main checkout's std and pattern tables.

Disk: every pair removes its `orb serve` scaffold (`.orbital/serve/<behavior>`, ~100 MB) and its resolved `.orb` once the
composite is written; each worker removes its Chrome profile and serve dir when its shard ends. A pair refuses to start
when the output volume has less than `H4_MIN_FREE_GB` (default 50) free.
