//! Native behavior catalog.
//!
//! Usage:
//!   orbital-native-catalog [--css <file.css>]...
//!
//! Opens ONE winit window (Blitz: Stylo + Taffy + vello) with a sidebar of
//! every baked behavior; clicking a row mounts its `.orb` through the SAME
//! pipeline `orb verify` runs (`shell::mount_core` → `KernelBridge` +
//! `ClientCore`), and event chips drive the state machine. With no `--css` flag it paints with the
//! renderer's own stylesheets (`styles::stylesheets()`: the `@almadar/ui` theme bundle, the base layer, the
//! Tailwind utilities); each `--css <file>` replaces that stack.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut css_paths: Vec<String> = Vec::new();
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
            _ => {
                eprintln!("unknown argument: {arg}");
                std::process::exit(2);
            }
        }
    }
    let css_texts: Vec<String> = if css_paths.is_empty() {
        orbital_client_dioxus::styles::stylesheets()
    } else {
        css_paths
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("failed to read css {p}: {e}")))
            .collect()
    };

    orbital_client_dioxus::app::CSS_TEXT.with(|c| *c.borrow_mut() = css_texts.clone());

    let attrs = dioxus_native::WindowAttributes::default()
        .with_title("Orbital · Native Catalog".to_string())
        .with_inner_size(dioxus_native::LogicalSize::new(1400.0, 900.0));
    let cfg = dioxus_native::Config::new().with_window_attributes(attrs);
    tracing::info!(
        target: orbital_client_dioxus::shell::NS,
        css_sheets = css_texts.len(),
        "native catalog launch"
    );
    // Game tick driver (same as `native::start`): run the ClientCore
    // TickScheduler on the vsync cadence so ticked behaviors animate.
    dioxus_native::set_frame_hook(Some(Box::new(|| {
        if orbital_client_dioxus::shell::drive_ticks() {
            orbital_client_dioxus::app::poke_rerender();
        }
    })));
    dioxus_native::launch_cfg(
        orbital_client_dioxus::catalog::CatalogApp,
        vec![],
        vec![Box::new(cfg)],
    );
    dioxus_native::set_frame_hook(None);
}

/// wasm32 stub — this bin is desktop-only (see `catalog.rs`).
#[cfg(target_arch = "wasm32")]
fn main() {}
