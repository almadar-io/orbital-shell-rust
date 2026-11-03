<div align="center">

# orbital-shell-rust

**The Rust shell: generates a native Cargo project that runs a served program on the kernel, plus the desktop, terminal and game clients.**

[![CI](https://github.com/almadar-io/orbital-shell-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/almadar-io/orbital-shell-rust/actions/workflows/ci.yml) [![License: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-blue)](LICENSE)

</div>

The Rust shell turns a compiled Orbital program into a native Rust project: a host entry that runs the served program, and client entry points (desktop, terminal, game). It also builds `orb-desktop`, a GUI host for programs, from the Dioxus and ratatui renderers it carries.

## Where this fits

```mermaid
flowchart TB
  subgraph kernel["Kernel and substrate"]
    core["orbital-core<br/>IR · evaluator · kernel"]
    canvas["orbital-canvas<br/>2D/3D render primitives"]
  end
  subgraph lang["Language"]
    lolo["orbital-lolo<br/>.lolo → .orb"]
    compiler["orbital-compiler<br/>validate · codegen"]
  end
  subgraph hosts["Shells and hosts"]
    ts["orbital-shell-typescript<br/>web · node · @almadar/core"]
    rs["orbital-shell-rust<br/>desktop · tui"]
    py["orbital-shell-python"]
    ios["orbital-shell-ios"]
    android["orbital-shell-android"]
    os["orbital-shell-os"]
    wasm["orbital-wasm<br/>browser kernel"]
    ffi["orbital-ffi<br/>C ABI for mobile"]
    router["orbital-router"]
  end
  cli["orbital-cli<br/>the orb binary"]
  ws["orbital-rust<br/>workspace · cross-crate tests"]
  dist["orb<br/>npm · releases"]
  subgraph pkgs["Language packages"]
    std["almadar-std<br/>@almadar/std substrate"]
    beh["behaviors<br/>themed organisms"]
    langs["lolo-ar · lolo-sl<br/>language packs"]
  end
  subgraph apps["Apps and sites"]
    builder["builder (studio)"]
    kflow["kflow-academy"]
    nata["nata"]
    rawi["rawi"]
    site["almadar (websites)"]
  end
  core --> lolo & compiler & ts & rs & py & ios & android & os & wasm & ffi & router
  canvas --> ts & rs & ffi
  lolo --> compiler
  compiler --> cli
  ts & rs & py & ios & android & os --> cli
  cli --> dist
  ws -.pins.-> core & lolo & compiler & cli
  dist --> std
  std --> beh
  std --> langs
  beh --> builder & kflow & nata & rawi & site
  click core "https://github.com/almadar-io/orbital-core"
  click canvas "https://github.com/almadar-io/orbital-canvas"
  click lolo "https://github.com/almadar-io/orbital-lolo"
  click compiler "https://github.com/almadar-io/orbital-compiler"
  click ts "https://github.com/almadar-io/orbital-shell-typescript"
  click rs "https://github.com/almadar-io/orbital-shell-rust"
  click py "https://github.com/almadar-io/orbital-shell-python"
  click ios "https://github.com/almadar-io/orbital-shell-ios"
  click android "https://github.com/almadar-io/orbital-shell-android"
  click os "https://github.com/almadar-io/orbital-shell-os"
  click wasm "https://github.com/almadar-io/orbital-wasm"
  click ffi "https://github.com/almadar-io/orbital-ffi"
  click router "https://github.com/almadar-io/orbital-router"
  click cli "https://github.com/almadar-io/orbital-cli"
  click ws "https://github.com/almadar-io/orbital-rust"
  click dist "https://github.com/almadar-io/orb"
  click std "https://github.com/almadar-io/almadar-std"
  click beh "https://github.com/almadar-io/behaviors"
  click langs "https://github.com/almadar-io/lolo-ar"
  click builder "https://github.com/itechdom/almadar-studio"
  click kflow "https://github.com/almadar-io/almadar-kflow-academy"
  click nata "https://github.com/almadar-io/almadar-nata"
  click rawi "https://github.com/almadar-io/almadar-rawi"
  click site "https://github.com/almadar-io/almadar"
  classDef here fill:#2563eb,color:#fff,stroke:#1e40af,stroke-width:3px
  class rs here
```

| Upstream (this repo depends on) | Downstream (depends on this repo) |
|---|---|
| [orbital-core](https://github.com/almadar-io/orbital-core): IR, evaluator and kernel (`orbital-core-base`, `orbital-core-eval`) | [orbital-cli](https://github.com/almadar-io/orbital-cli): the `orb` binary, which runs this shell as `orb-shell-rust` for `orb compile --shell rust` |
|  | [orbital-shell-python](https://github.com/almadar-io/orbital-shell-python): its native extension (`bridges/native`) depends on this crate |

## Quick start

```bash
orb compile <program.orb> --shell rust -o <out-dir>
cd <out-dir> && cargo check && cargo run
```

## How `orb compile --shell rust` uses it

`orb` runs the `orb-shell-rust` program. It scaffolds `templates/rust-shell` (binaries `desktop`, `terminal`, `game`) and writes the served program for the host. The generated app runs on the native Rust kernel, `orbital-core`, through this crate's `host` module; the desktop client also depends on `orbital-core` with the `client-http` feature and on the Dioxus and TUI renderers in this repo. The template's `Cargo.toml` consumes these crates by relative path, so a generated app must be emitted inside the `orbital-rust` workspace (two levels under its root). The `renderer` option accepts `all`; `tui` is rejected as deferred (G-CROSS-290).

## Layout

| Path | Contents |
|---|---|
| `src/` | The crate: codegen (`desktop`, `game`, `server`, `tui`), shell, `orb-shell-rust` program, `orb-desktop` binary |
| `templates/rust-shell/`, `templates/rust-terminal/` | Project scaffolds the shell copies into every generated app |
| `renderers/dioxus/`, `renderers/tui/` | GUI (Dioxus) and terminal (ratatui) renderers |
| `hosts/` | `native`, `desktop`, `terminal`, `web`, `browser` host sources |
| `runtime/` | Dioxus runtime sources |
| `tests/` | `desktop_program.rs` and fixtures |

`orb-desktop` is behind the `desktop` Cargo feature (`required-features = ["desktop"]`).

## Development

The crate reads the Almadar language data (patterns, operators, themes, vocabularies) from the installed npm packages, so install those first (this is the CI recipe):

```bash
npm install --no-save --no-package-lock @almadar/core@^10 @almadar/std@^16 @almadar/lolo-ar@^1 @almadar/lolo-sl@^1
cargo test
```

Open gaps are tracked in [Almadar_Gaps.md](Almadar_Gaps.md). Work on `main`; never create a feature branch.

## Docs

- [Orb documentation](https://orb.almadar.io/docs): the language, the `orb` CLI and how the pieces fit

## License

[Business Source License 1.1](LICENSE): free for non-commercial and evaluation use; converts to Apache-2.0 on the change date.
