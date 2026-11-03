//! Desktop preview for the dioxus-native (Blitz) render path.
//!
//! Usage:
//!   orbital-native-preview <schema.orb> [--css <file.css>]... [--theme NAME] [--events A,B,C] [--event NAME PAYLOAD_JSON]... [--capture-gpu PNG WIDTH HEIGHT]
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
    let mut events: Vec<(String, String)> = Vec::new();
    let mut gpu_capture: Option<(String, u32, u32)> = None;
    let mut export_client: Option<String> = None;
    let mut theme: Option<String> = None;
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
                events.extend(
                    e.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|name| (name.to_string(), String::new())),
                );
            }
            "--event" => {
                let (Some(name), Some(payload)) = (args.next(), args.next()) else {
                    eprintln!("--event requires a name and JSON payload");
                    std::process::exit(2);
                };
                events.push((name, payload));
            }
            "--capture-gpu" => {
                let (Some(path), Some(width), Some(height)) = (args.next(), args.next(), args.next()) else {
                    eprintln!("--capture-gpu requires a PNG path, width, and height");
                    std::process::exit(2);
                };
                let width = width.parse::<u32>().ok().filter(|n| *n > 0).expect("positive capture width");
                let height = height.parse::<u32>().ok().filter(|n| *n > 0).expect("positive capture height");
                gpu_capture = Some((path, width, height));
            }
            "--export-client" => {
                export_client = Some(args.next().expect("--export-client requires a path"));
            }
            "--theme" => {
                theme = Some(args.next().expect("--theme requires a name"));
            }
            _ if arg.starts_with("--") => {
                eprintln!("unknown flag: {arg}");
                std::process::exit(2);
            }
            _ => schema_path = Some(arg),
        }
    }
    let Some(schema_path) = schema_path else {
        eprintln!("usage: orbital-native-preview <schema.orb> [--css <file.css>]... [--theme NAME] [--events A,B,C] [--event NAME PAYLOAD_JSON]... [--capture-gpu PNG WIDTH HEIGHT]");
        std::process::exit(2);
    };

    orbital_client_dioxus::app::THEME.with(|current| *current.borrow_mut() = theme);

    let schema_json = orbital_client_dioxus::shell::orb_resolve(std::path::Path::new(&schema_path))
        .unwrap_or_else(|e| panic!("failed to resolve {schema_path}: {e}"));
    if let Some(path) = export_client {
        let mut program: serde_json::Value = serde_json::from_str(&schema_json).expect("resolved program JSON");
        let metadata = orbital_core::effects::runs_on::client_program_metadata(&program["orbitals"]);
        let root = program.as_object_mut().expect("resolved program object");
        let slot = root.entry("_metadata").or_insert_with(|| serde_json::json!({}));
        slot.as_object_mut().expect("metadata object").extend(metadata);
        let language = orbital_core::language_data::language_bundle_json().expect("installed language bundle");
        let language_path = std::path::Path::new(&path).with_file_name("language.json");
        std::fs::write(language_path, language).expect("write language bundle");
        std::fs::write(path, serde_json::to_vec(&program).expect("client program JSON"))
            .expect("write client program");
        return;
    }
    let css_texts: Vec<String> = css_paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(p).unwrap_or_else(|e| panic!("failed to read css {p}: {e}"))
        })
        .collect();

    if !events.is_empty() || gpu_capture.is_some() {
        // Scripted-event mode: mount, fire the list, print the snapshot — no
        // window (the headless harness grows from this).
        orbital_client_dioxus::shell::mount_core(&schema_json)
            .unwrap_or_else(|e| panic!("mount failed: {e}"));
        for (event, payload) in &events {
            orbital_client_dioxus::shell::drive_event(event, payload)
                .unwrap_or_else(|e| panic!("event {event} failed: {e}"));
        }
        if let Some((path, width, height)) = gpu_capture {
            orbital_client_dioxus::app::CSS_TEXT.with(|cell| *cell.borrow_mut() = css_texts);
            orbital_client_dioxus::offscreen::capture_gpu_png(std::path::Path::new(&path), width, height)
                .unwrap_or_else(|e| panic!("GPU capture {path} failed: {e}"));
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
