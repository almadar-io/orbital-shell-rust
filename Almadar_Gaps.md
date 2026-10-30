<!-- Gap ledger for this repo: the source of truth for its open gaps. -->
# orbital-shell-rust — open gaps

Every open gap this repo owns lives here. This file is the source of truth; the monorepo's `docs/Almadar_Gaps.md` only rolls it up.

- **One entry per gap:** `- **<code>** — <what is wrong and where>. <owning package> [mechanical|architectural] — <evidence, prevention rung>`.
- **Codes:** new gaps use this repo's prefix `G-ORBITAL-SHELL-RUST-`. Take the "Next code" below, then bump it in the same edit. Codes are never reused or renamed.
- **Close by deleting.** Remove the entry in the same commit as the fix.

Next code: `G-ORBITAL-SHELL-RUST-004`

- **G-ORBITAL-SHELL-RUST-002** [mechanical] — The Rust shell keeps host decisions and retired code (R2 shell survey 2026-10-29): runtime/src/runtime.rs tick_pass (:829-850) gates ticks on `background` in the host (node asks engine.tickPass); PersistenceEntitySource.write (:386-405) picks update vs create by fallback; hosts/native/src/store.rs DriverAdapter (:82-308) is only built via memory(), its driver constructor unused (drivers retired) and row_matches (:118-126) filters rows in the host; Cargo.toml still enables orbital-core/client-http for `desktop`. Root cause: K3.

- **G-ORB-238** — The Dioxus renderer (`renderers/dioxus`) stubs 27 registry patterns, declared in `registry::STUBBED_PATTERNS` (owner 2026-10-11: stub now, implement later): action-palette, action-tile, choice-button, control-button, control-grid, dialogue-bubble, draw-fx-layer, draw-group, draw-mesh, draw-shape, draw-shape-layer, draw-skinned-mesh, draw-sprite, draw-sprite-layer, draw-text, draw-text-layer, game-audio-cue, game-audio-toggle, game-menu, game-shell, health-bar, pattern-tile, score-display, sequence-bar, stat-badge, timer-display, trait-slot. Each draws the declared `ORB_PATTERN_UNSUPPORTED_BY_RENDERER` answer, and `orb verify`'s Layer 2 lists them as stubbed, not failed. `tests/renderer_coverage.rs` keeps the list equal to what the renderer lacks. Fix: a Dioxus body per pattern in `generate-rust-renderers.ts` `NATIVE_TEMPLATES` (`--backend dioxus`), dropping each from the list as it lands. Rung: none of the language rungs — renderer coverage. [mechanical] Root cause: H4.
