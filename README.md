# orbital-shell-rust

The Rust shell: generates an Axum backend from a compiled Orbital program.

Part of [Almadar](https://almadar.io)'s Orbital compiler, which turns a `.lolo`/`.orb` program into a full-stack app. The `orb` CLI (npm `@almadar/orb`) bundles this crate; you only need this repo to read, build or change it.

Depends on [`orbital-core`](https://github.com/almadar-io/orbital-core).

## Layout

- `src/` — the crate.
- `template/` — the project scaffold the shell copies into every generated app.

## Build and test

The crate reads the Almadar language data (patterns, operators, themes, vocabularies) from the installed npm packages, so install those first:

```bash
npm install --no-save --no-package-lock @almadar/core@^10 @almadar/std@^16 @almadar/lolo-ar@^1 @almadar/lolo-sl@^1
cargo test
```

## License

[Business Source License 1.1](LICENSE) — free for non-commercial and evaluation use; converts to Apache-2.0 on the change date.
