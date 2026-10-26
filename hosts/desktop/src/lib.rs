//! `orb-desktop`: the native GUI as a program (`orbital_core::desktop_protocol`).
//! It hosts this shell's renderers — Dioxus (the window, PNG screenshots) and
//! the terminal (text frames) — picked per request by name.

use orbital_client_dioxus::{offscreen, shell as dioxus};
use orbital_client_tui::TuiApp;
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::client_core::ClientCore;
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

/// An in-process kernel over the app's own rows.
fn kernel(schema: &OrbitalSchema) -> Result<Arc<dyn ServerBridge>, String> {
    let kernel = KernelBridge::new(schema.clone()).map_err(|e| format!("kernel bridge: {e}"))?;
    Ok(Arc::new(kernel))
}

/// One mounted app on one renderer: drive its circuit, capture what it shows.
enum Surface {
    Dioxus,
    Tui(Box<TuiApp>),
}

impl Surface {
    fn mount(renderer: Renderer, schema: &OrbitalSchema, bridge: Arc<dyn ServerBridge>) -> Result<Self, String> {
        Ok(match renderer {
            Renderer::Dioxus => {
                dioxus::mount(schema.clone(), bridge);
                orbital_client_dioxus::app::CSS_TEXT.with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
                Surface::Dioxus
            }
            Renderer::Tui => Surface::Tui(Box::new(TuiApp::new(schema.clone(), Some(bridge), Vec::new()).map_err(|e| format!("{e:#}"))?)),
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
fn screenshot(renderer: Renderer, item: &str, schema: &OrbitalSchema, out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut surface = Surface::mount(renderer, schema, kernel(schema)?)?;
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

fn coverage(renderer: Renderer) -> DesktopReply {
    match renderer {
        Renderer::Dioxus => {
            let registry = orbital_client_dioxus::registry::default_registry();
            DesktopReply::Coverage {
                supported: registry.coverage().into_iter().map(str::to_string).collect(),
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
            DesktopRequest::Screenshot { renderer, item, schema, out_dir } => {
                screenshot(Renderer::parse(&renderer)?, &item, &schema, &out_dir).map(DesktopReply::Files).map_err(failed)
            }
        }
    }

    fn open_window(&self, renderer: &str, schema_path: &Path, server: Option<&str>) -> Result<(), String> {
        match Renderer::parse(renderer).map_err(|f| f.to_string())? {
            Renderer::Tui => orbital_client_tui::run(schema_path, server).map_err(|e| format!("{e:#}")),
            Renderer::Dioxus => {
                let json = std::fs::read_to_string(schema_path).map_err(|e| format!("{}: {e}", schema_path.display()))?;
                let title = match server {
                    Some(url) => {
                        let schema = orbital_core::schema::parse_schema(&json).map_err(|e| e.to_string())?;
                        let title = schema.name.clone();
                        dioxus::mount(schema, Arc::new(SyncServerBridge::new(url)));
                        title
                    }
                    None => {
                        dioxus::mount_core(&json)?;
                        orbital_core::schema::parse_schema(&json).map_err(|e| e.to_string())?.name
                    }
                };
                orbital_client_dioxus::native::open_window(orbital_client_dioxus::styles::stylesheets(), &title);
                Ok(())
            }
        }
    }
}
