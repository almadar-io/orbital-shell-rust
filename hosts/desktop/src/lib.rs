//! `orb-desktop`: the native GUI as a program (`orbital_core::desktop_protocol`).
//! It hosts this shell's renderers — Dioxus (the window, PNG screenshots) and
//! the terminal (text frames) — picked per request by name.

use orbital_client_dioxus::{offscreen, shell as dioxus};
use orbital_client_tui::TuiApp;
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::client_core::ClientCore;
use orbital_core::client::client_kernel::ClientKernel;
use orbital_core::client::http::SyncServerBridge;
use orbital_core::client::render_probe::{GameCanvasOutcome, RenderTarget};
use orbital_core::desktop_protocol::{DesktopProgram, DesktopReply, DesktopRequest, RENDERERS};
use orbital_core::shell_protocol::{ShellFailure, CODE_BAD_REQUEST, CODE_GENERATE_FAILED};
use orbital_core::OrbitalSchema;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Desktop;

/// Screenshot size, the egui client's default window.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 900;
const TEXT_COLUMNS: u16 = 160;
const TEXT_ROWS: u16 = 50;
/// A route capture runs at most this many host frames, `FRAME_MS` apart, before painting.
const FRAME_LIMIT: usize = 40;
const FRAME_MS: u64 = 50;

fn failed(message: impl Into<String>) -> ShellFailure {
    ShellFailure::new(CODE_GENERATE_FAILED, message)
}

#[derive(Clone, Copy)]
enum Renderer {
    Dioxus,
    Tui,
}

impl Renderer {
    fn parse(name: &str) -> Result<Self, ShellFailure> {
        match name {
            "dioxus" => Ok(Renderer::Dioxus),
            "tui" => Ok(Renderer::Tui),
            other => Err(ShellFailure::new(CODE_BAD_REQUEST, format!("unknown renderer {other:?} (expected one of {})", RENDERERS.join(", ")))),
        }
    }
}

/// The program a client-role kernel runs against a server: the same projection the browser receives (EXEC-ENDPOINT-5),
/// so the server's boot and protocol orbitals stay on the server.
fn client_schema(schema: &OrbitalSchema) -> Result<OrbitalSchema, String> {
    let program = serde_json::to_value(schema).map_err(|e| format!("client program: {e}"))?;
    serde_json::from_value(orbital_core::effects::runs_on::client_program(&program)).map_err(|e| format!("client program: {e}"))
}

/// The viewer's locales and their catalogs (locale, then qualified key, to message).
type Messages = (Vec<String>, std::collections::HashMap<String, std::collections::HashMap<String, String>>);

/// The catalogs of a program that declares `locales`, read from beside its resolved `.orb` as the served host reads
/// them. A program without locales has none; one with locales and no path to read them from is refused, not
/// painted with empty messages.
fn messages_of(schema: &OrbitalSchema, program_path: Option<&Path>) -> Result<Option<Messages>, String> {
    let Some(locales) = schema.locales.clone() else {
        return Ok(None);
    };
    let path = program_path.ok_or_else(|| format!("{} declares locales {locales:?} but no program path says where their catalogs are", schema.name))?;
    let catalogs = crate::host::boot::read_catalogs(path, &locales).map_err(|e| e.to_string())?;
    Ok(Some((locales, catalogs)))
}

/// An in-process kernel over the app's own rows.
fn kernel(schema: &OrbitalSchema, messages: Option<&Messages>) -> Result<Arc<dyn ServerBridge>, String> {
    let kernel = KernelBridge::new(schema.clone()).map_err(|e| format!("kernel bridge: {e}"))?;
    if let Some((locales, catalogs)) = messages {
        kernel.set_messages(locales.clone(), catalogs.clone());
    }
    Ok(Arc::new(kernel))
}

/// The client-role circuit over `bridge`, with the viewer's messages installed before its first INIT.
fn circuit(schema: &OrbitalSchema, bridge: Arc<dyn ServerBridge>, messages: Option<&Messages>) -> Result<ClientCore, String> {
    let kernel = ClientKernel::new(schema.clone(), Some(bridge)).map_err(|e| format!("client kernel: {e}"))?;
    if let Some((locales, catalogs)) = messages {
        kernel.set_messages(locales.clone(), catalogs.clone());
    }
    Ok(ClientCore::with_client_kernel(schema.clone(), Arc::new(kernel)))
}

/// One mounted app on one renderer: drive its circuit, capture what it shows.
enum Surface {
    Dioxus,
    Tui(Box<TuiApp>),
}

impl Surface {
    fn mount(renderer: Renderer, schema: &OrbitalSchema, bridge: Arc<dyn ServerBridge>, messages: Option<&Messages>) -> Result<Self, String> {
        let core = circuit(schema, bridge, messages)?;
        Ok(match renderer {
            Renderer::Dioxus => {
                dioxus::mount_circuit(core);
                orbital_client_dioxus::app::CSS_TEXT.with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::program_stylesheets(schema));
                Surface::Dioxus
            }
            Renderer::Tui => Surface::Tui(Box::new(TuiApp::with_core(core, Vec::new()))),
        })
    }

    fn drive<R>(&mut self, f: impl FnOnce(&mut ClientCore) -> R) -> Result<R, String> {
        match self {
            Surface::Dioxus => dioxus::with_core(f),
            Surface::Tui(app) => Ok(f(app.core_mut())),
        }
    }

    /// Capture to `stem` + the renderer's extension (`.png` / `.txt`).
    fn capture(&mut self, stem: &Path) -> Result<PathBuf, String> {
        match self {
            Surface::Dioxus => {
                let path = PathBuf::from(format!("{}.png", stem.display()));
                offscreen::capture_png(&path, WIDTH, HEIGHT)?;
                Ok(path)
            }
            Surface::Tui(app) => {
                let path = PathBuf::from(format!("{}.txt", stem.display()));
                let text = app.render_to_text(TEXT_COLUMNS, TEXT_ROWS).map_err(|e| format!("{e:#}"))?;
                std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(path)
            }
        }
    }
}

/// Every orbital, initialised, captured once.
fn screenshot(renderer: Renderer, item: &str, schema: &OrbitalSchema, out_dir: &Path, program_path: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let messages = messages_of(schema, program_path)?;
    let mut surface = Surface::mount(renderer, schema, kernel(schema, messages.as_ref())?, messages.as_ref())?;
    let mut written = Vec::new();
    for idx in 0..schema.orbitals.len().max(1) {
        surface.drive(|core| {
            core.set_orbital(idx);
            core.init_all_traits_blocking();
            core.settle(8);
        })?;
        written.push(surface.capture(&out_dir.join(format!("{item}.orbital{idx}")))?);
    }
    Ok(written)
}

/// The app as a route mount opens `route` (its `$MOUNT` first, then the page's seeds), settled, captured once.
fn screenshot_route(
    renderer: Renderer,
    item: &str,
    schema: &OrbitalSchema,
    out_dir: &Path,
    route: &str,
    server: Option<&str>,
    program_path: Option<&Path>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let messages = messages_of(schema, program_path)?;
    let (schema, bridge): (OrbitalSchema, Arc<dyn ServerBridge>) = match server {
        Some(url) => (client_schema(schema)?, Arc::new(SyncServerBridge::new(url))),
        None => (schema.clone(), kernel(schema, messages.as_ref())?),
    };
    let mut surface = Surface::mount(renderer, &schema, bridge, messages.as_ref())?;
    let painted = surface.drive(|core| {
        // The window's first mount already opened the program's first page; another route is opened as navigation.
        if core.page_mount_route().as_deref() != Some(route) {
            core.navigate_to(route, None);
            core.mount_page_blocking();
        }
        core.settle(8);
        // The window's frame loop: run frames until nothing ran and nothing awaits (bounded), as the app would paint.
        for _ in 0..FRAME_LIMIT {
            let ran = core.drive_ticks_blocking();
            core.settle(8);
            if !ran && !core.has_awaiting() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(FRAME_MS));
        }
        painted_tree(core)
    })?;
    let tree = out_dir.join(format!("{item}.tree.json"));
    let text = serde_json::to_string_pretty(&painted).map_err(|e| e.to_string())?;
    std::fs::write(&tree, text).map_err(|e| format!("{}: {e}", tree.display()))?;
    surface.capture(&out_dir.join(item))
}

/// What the capture painted, beside the image: each slot's render tree (as the kernel handed it), the active trait's
/// state and any status message, so a parity review reads the desktop's input instead of guessing from pixels.
fn painted_tree(core: &ClientCore) -> serde_json::Value {
    let mut slots = serde_json::Map::new();
    let mut names: Vec<&String> = core.ui_slots.keys().collect();
    names.sort();
    for name in names {
        let items: Vec<serde_json::Value> = core.ui_slots[name]
            .iter()
            .map(|content| {
                serde_json::json!({
                    "pattern": content.pattern,
                    "sourceTrait": content.source_trait,
                    "props": orbital_core::client::helpers::to_json(&content.props),
                })
            })
            .collect();
        slots.insert(name.clone(), serde_json::Value::Array(items));
    }
    serde_json::json!({
        "trait": core.current_trait_name,
        "state": core.current_state,
        "status": core.status_message.as_ref().map(|(text, error)| serde_json::json!({ "text": text, "error": error })),
        "slots": slots,
    })
}

fn coverage(renderer: Renderer) -> DesktopReply {
    match renderer {
        Renderer::Dioxus => {
            let registry = orbital_client_dioxus::registry::default_registry();
            DesktopReply::Coverage {
                supported: registry.complete_coverage().into_iter().map(str::to_string).collect(),
                stubbed: orbital_client_dioxus::registry::STUBBED_PATTERNS.iter().map(|s| s.to_string()).collect(),
            }
        }
        Renderer::Tui => {
            let registry = orbital_client_tui::PatternRegistry::new();
            DesktopReply::Coverage { supported: registry.coverage().into_iter().map(str::to_string).collect(), stubbed: Vec::new() }
        }
    }
}

impl DesktopProgram for Desktop {
    fn handle(&self, request: DesktopRequest) -> Result<DesktopReply, ShellFailure> {
        match request {
            DesktopRequest::Coverage { renderer } => Ok(coverage(Renderer::parse(&renderer)?)),
            DesktopRequest::PaintCanvas { renderer, pattern, props } => Ok(DesktopReply::Canvas(match Renderer::parse(&renderer)? {
                Renderer::Dioxus => orbital_client_dioxus::probe_target::DioxusTarget::new().probe_game_canvas(&pattern, &props),
                Renderer::Tui => GameCanvasOutcome::Skip(format!("`{pattern}`: the terminal renderer paints no canvases")),
            })),
            DesktopRequest::Screenshot { renderer, item, schema, out_dir, route, server, program_path } => match route {
                Some(route) => screenshot_route(Renderer::parse(&renderer)?, &item, &schema, &out_dir, &route, server.as_deref(), program_path.as_deref())
                    .map(|file| DesktopReply::Files(vec![file]))
                    .map_err(failed),
                None => screenshot(Renderer::parse(&renderer)?, &item, &schema, &out_dir, program_path.as_deref()).map(DesktopReply::Files).map_err(failed),
            },
        }
    }

    fn open_window(&self, renderer: &str, schema_path: &Path, server: Option<&str>) -> Result<(), String> {
        match Renderer::parse(renderer).map_err(|f| f.to_string())? {
            Renderer::Tui => orbital_client_tui::run(schema_path, server).map_err(|e| format!("{e:#}")),
            Renderer::Dioxus => {
                let json = std::fs::read_to_string(schema_path).map_err(|e| format!("{}: {e}", schema_path.display()))?;
                let schema = orbital_core::schema::parse_schema(&json).map_err(|e| e.to_string())?;
                let title = schema.name.clone();
                let css = orbital_client_dioxus::styles::program_stylesheets(&schema);
                let messages = messages_of(&schema, Some(schema_path))?;
                let (circuit_schema, bridge): (OrbitalSchema, Arc<dyn ServerBridge>) = match server {
                    Some(url) => (client_schema(&schema)?, Arc::new(SyncServerBridge::new(url))),
                    None => (schema.clone(), kernel(&schema, messages.as_ref())?),
                };
                dioxus::mount_circuit(circuit(&circuit_schema, bridge, messages.as_ref())?);
                orbital_client_dioxus::native::open_window(css, &title);
                Ok(())
            }
        }
    }
}
