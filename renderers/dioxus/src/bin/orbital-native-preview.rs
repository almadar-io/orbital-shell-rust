//! Desktop preview for the dioxus-native (Blitz) render path.
//!
//! Usage:
//!   orbital-native-preview <schema.orb> [--css <file.css>]... [--events A,B,C]
//!
//! Resolves the schema with `orb resolve` (the pipeline `orb verify` runs),
//! seeds singleton entities, opens a Blitz window, and optionally fires a
//! comma-separated event list after mount (for scripted checks). CSS files
//! are passed by the caller — typically `@almadar/ui-wasm`'s
//! `preview/almadar.css` + `dist/tailwind.css`.
//!
//! The bin is desktop-only: on wasm32 the crate's `native` module is cfg'd
//! out (the browser glue lives in `src/wasm.rs`), so a stub `main` keeps
//! `cargo build --target wasm32-unknown-unknown` (which builds all targets of
//! the package) from failing on the missing module.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Log level from RUST_LOG (default info) — canvas/mutation forensics need
    // `RUST_LOG="dioxus_native_dom=trace,blitz_dom=debug"`.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_target(true)
        .with_env_filter(filter)
        .init();

    let mut schema_path: Option<String> = None;
    let mut css_paths: Vec<String> = Vec::new();
    let mut events: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--css" => {
                let Some(p) = args.next() else {
                    eprintln!("--css requires a path");
                    std::process::exit(2);
                };
                css_paths.push(p);
            }
            "--events" => {
                let Some(e) = args.next() else {
                    eprintln!("--events requires a comma-separated list");
                    std::process::exit(2);
                };
                events.extend(e.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string));
            }
            _ if arg.starts_with("--") => {
                eprintln!("unknown flag: {arg}");
                std::process::exit(2);
            }
            _ => schema_path = Some(arg),
        }
    }
    let Some(schema_path) = schema_path else {
        eprintln!("usage: orbital-native-preview <schema.orb> [--css <file.css>]... [--events A,B,C]");
        std::process::exit(2);
    };

    let schema_json = orbital_client_dioxus::shell::orb_resolve(std::path::Path::new(&schema_path))
        .unwrap_or_else(|e| panic!("failed to resolve {schema_path}: {e}"));
    let css_texts: Vec<String> = css_paths
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("failed to read css {p}: {e}")))
        .collect();

    if !events.is_empty() {
        // Scripted-event mode: mount, fire the list, print the snapshot — no
        // window (the headless harness grows from this).
        orbital_client_dioxus::shell::mount_core(&schema_json)
            .unwrap_or_else(|e| panic!("mount failed: {e}"));
        for ev in &events {
            orbital_client_dioxus::shell::drive_event(ev, "")
                .unwrap_or_else(|e| panic!("event {ev} failed: {e}"));
        }
        let snap = orbital_client_dioxus::shell::snapshot_json().expect("snapshot after mount");
        println!("{snap}");
        return;
    }

    let title = format!(
        "Orbital · Dioxus Native — {}",
        std::path::Path::new(&schema_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("schema")
    );
    orbital_client_dioxus::native::start(&schema_json, css_texts, &title)
        .unwrap_or_else(|e| panic!("native start failed: {e}"));
}

/// wasm32 stub — see the module docs. The browser entry point is the
/// `WebHandle` bridge in `src/wasm.rs`, not this bin.
#[cfg(target_arch = "wasm32")]
fn main() {}
