//! Browser (wasm32) mount surface for the NATIVE (Blitz) render path — the
//! egui-shell model, not DOM: the same Dioxus VDOM + `App` component as the
//! desktop `native` shell, but painted by Blitz (Stylo CSS + Taffy layout +
//! Parley text) into a vello/wgpu scene presented to a `<canvas>`. Nothing is
//! inserted into the page DOM except the canvas element itself.
//!
//! blitz-shell's winit event loop is cfg'd desktop-only upstream, so this
//! module replicates ~200 lines of it for the browser:
//!   - `DioxusDocument::new(VirtualDom::new(App), DocumentConfig)` — the SAME
//!     VDOM→Blitz bridge dioxus-native uses on desktop (`resumed` there).
//!   - A `document::Style` head-element sink (`WasmHeadSink`) so the CSS text
//!     handed to `start` reaches Stylo exactly like the desktop `--css` path.
//!   - wgpu surface init via `spawn_local` (WebGPU adapter/device acquisition
//!     is promise-based; `pollster::block_on` — used by anyrender-vello's
//!     `resume` — deadlocks on wasm because microtasks never run inside it).
//!   - A `requestAnimationFrame` loop: drain head ops → `doc.poll` (flushes
//!     VDOM work, incl. the version-signal poke from `send_event`) → paint
//!     with `blitz_paint::paint_scene` → `render_to_texture` → present.
//!   - Mouse input: web-sys listeners → `blitz_traits::events::UiEvent` →
//!     `doc.handle_ui_event` (which runs the Dioxus handlers → `EventSink` →
//!     the shared `ClientCore` circuit). Keyboard/IME is a deliberate
//!     follow-up — the current gate (atoms with buttons) is mouse-only.
//!
//! JS API (consumed by `@almadar/ui-wasm`'s TS bridge):
//! `new WebHandle()` · `start(elementId, schemaJson, cssText)` ·
//! `send_event(event, payloadJson)` · `snapshot()` · `current_state()` ·
//! `destroy()`.

#![cfg(all(feature = "native", target_arch = "wasm32"))]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use anyrender::ImageRenderer;
use anyrender_vello::wgpu::{self, SurfaceTarget};
use anyrender_vello::VelloScenePainter;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{Document as BlitzDocument, DocumentConfig};
use blitz_paint::paint_scene;
use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_core::{provide_context, ScopeId, VirtualDom};
use dioxus_native::DioxusDocument;
use keyboard_types::Modifiers;
use vello::peniko::Color;
use vello::{AaConfig, AaSupport, RenderParams, Renderer as VelloRenderer, RendererOptions, Scene};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, MouseEvent};
use wgpu_context::{SurfaceRenderer, SurfaceRendererConfiguration, TextureConfiguration, WGPUContext};

use crate::app::{poke_rerender, App, CSS_TEXT};
use crate::shell;

/// Log namespace — mirrors `@almadar/logger` namespaces on the TS side.
const NS: &str = "almadar::ui_wasm_dioxus";

static LOG_INIT: std::sync::Once = std::sync::Once::new();

// ---------------------------------------------------------------------------
// Logging: tracing → log → console_log (same shape as the egui wasm shell)
// ---------------------------------------------------------------------------

struct ConsoleSubscriber;

struct FieldVisitor(String);
impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn core::fmt::Debug) {
        use core::fmt::Write;
        let _ = write!(&mut self.0, " {}={:?}", field.name(), value);
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        use core::fmt::Write;
        let _ = write!(&mut self.0, " {}={}", field.name(), value);
    }
}

impl tracing::Subscriber for ConsoleSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let meta = event.metadata();
        // Stylo/Blitz internals emit whole stylesheets per frame at debug —
        // same quiet list as FilteringLogger (their `tracing` events arrive
        // here, not through the `log` facade).
        if *meta.level() > tracing::Level::INFO
            && QUIET_TARGETS.iter().any(|q| meta.target().starts_with(q))
        {
            return;
        }
        let mut visitor = FieldVisitor(String::new());
        event.record(&mut visitor);
        let level = match *meta.level() {
            tracing::Level::ERROR => log::Level::Error,
            tracing::Level::WARN => log::Level::Warn,
            tracing::Level::INFO => log::Level::Info,
            tracing::Level::DEBUG => log::Level::Debug,
            tracing::Level::TRACE => log::Level::Trace,
        };
        log::log!(target: "almadar::trace", level, "[{}]{}", meta.target(), visitor.0);
    }
    fn enter(&self, _span: &tracing::span::Id) {}
    fn exit(&self, _span: &tracing::span::Id) {}
}

/// `log` facade impl for wasm: forwards to the browser console (same shape as
/// `console_log`) but drops debug/trace from the Stylo/Blitz layout internals,
/// which dump whole stylesheets per frame and drown the namespaced almadar
/// logs. Our own targets (`almadar::*`, `orbital_*`) always pass.
struct FilteringLogger;

/// Targets whose debug/trace output is pure layout noise on wasm.
/// (`style` is servo's stylo crate — its module-path targets are `style::*`.)
const QUIET_TARGETS: &[&str] = &[
    "stylo",
    "style",
    "selectors",
    "blitz_dom",
    "blitz_paint",
    "blitz_traits",
];

impl log::Log for FilteringLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.level() <= log::Level::Info {
            return true;
        }
        let target = metadata.target();
        !QUIET_TARGETS.iter().any(|q| target.starts_with(q))
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let msg = format!("{}", record.args());
        let js = wasm_bindgen::JsValue::from_str(&msg);
        match record.level() {
            log::Level::Error => web_sys::console::error_1(&js),
            log::Level::Warn => web_sys::console::warn_1(&js),
            log::Level::Info => web_sys::console::info_1(&js),
            log::Level::Debug => web_sys::console::log_1(&js),
            log::Level::Trace => web_sys::console::debug_1(&js),
        }
    }

    fn flush(&self) {}
}

static FILTERING_LOGGER: FilteringLogger = FilteringLogger;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

enum RendererState {
    /// wgpu surface/adapter/device acquisition in flight (spawn_local).
    Pending,
    /// GPU path: vello over WebGPU — preferred. The `VelloRenderer` itself is
    /// NOT here: it lives in `GPU_CACHE` (shader pipelines are process-wide,
    /// shared across mounts); only the per-canvas surface is per-mount.
    Active { surface: SurfaceRenderer<'static> },
    /// CPU path: vello_cpu rasterizes the same scene into an RGBA buffer,
    /// blitted with `putImageData`. For browsers without WebGPU (egui worked
    /// there because WebGL2 exists everywhere — vello needs compute shaders,
    /// which only WebGPU exposes). The canvas is a FRESH element: a canvas
    /// probed by wgpu's `getContext('webgpu')` is locked to that context type.
    Cpu {
        renderer: VelloCpuImageRenderer,
        ctx: CanvasRenderingContext2d,
        /// RGBA8 backing, `width * height * 4`; refilled white each paint.
        buffer: Vec<u8>,
    },
    /// Logged once; frames keep polling the VDOM so state still advances.
    Failed,
}

/// DOM listener closures, kept alive here and dropped with the app.
struct Listeners {
    on_mouse_move: Closure<dyn FnMut(MouseEvent)>,
    on_mouse_down: Closure<dyn FnMut(MouseEvent)>,
    on_mouse_up: Closure<dyn FnMut(MouseEvent)>,
    /// Window-scoped (never re-registered), held only to keep it alive.
    #[allow(dead_code)]
    on_resize: Closure<dyn FnMut()>,
}

struct WasmApp {
    doc: DioxusDocument,
    scene: Scene,
    renderer: RendererState,
    canvas: HtmlCanvasElement,
    /// Physical pixels (canvas backing store).
    width: u32,
    height: u32,
    scale: f32,
    dirty: bool,
    /// Why the current dirty flag was raised (bits in `mod dirty`).
    dirty_reasons: u32,
    /// Frame-phase timing accumulator (spike + periodic perf logs).
    perf: FramePerf,
    alive: bool,
    /// One-shot gate for the layout tree dump (first resolved frame).
    layout_dumped: bool,
    head_ops: Rc<RefCell<VecDeque<crate::offscreen::HeadOp>>>,
    listeners: Option<Listeners>,
    raf: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>,
}

std::thread_local! {
    static WASM_APP: RefCell<Option<Rc<RefCell<WasmApp>>>> = const { RefCell::new(None) };
    /// Process-wide GPU resources shared across mounts. A new `VelloRenderer`
    /// compiles the full vello shader pipeline on the driver — seconds on a
    /// real GPU — so the context + renderer are created once and reused; only
    /// the surface (bound to each mount's fresh canvas) is per-mount.
    static GPU_CACHE: RefCell<Option<GpuCache>> = const { RefCell::new(None) };
    /// Set when surface acquisition or vello init failed — subsequent mounts
    /// go straight to the CPU rasterizer instead of paying the multi-second
    /// adapter/shader attempt on every catalog click.
    static GPU_UNAVAILABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct GpuCache {
    wgpu_ctx: WGPUContext,
    vello: VelloRenderer,
}

fn with_app<R>(f: impl FnOnce(&mut WasmApp) -> R) -> Option<R> {
    let app_rc = WASM_APP.with(|cell| cell.borrow().as_ref().cloned())?;
    let mut app = app_rc.borrow_mut();
    Some(f(&mut app))
}


/// Recursive layout dump for diagnosing collapsed/zero-size subtrees. Logs
/// tag, class, text prefix, and final Taffy rect per node — the ground truth
/// for "why did nothing paint".
fn dump_layout(doc: &DioxusDocument, id: usize, depth: usize, out: &mut String) {
    use core::fmt::Write;
    let Some(node) = doc.get_node(id) else { return };
    let l = &node.final_layout;
    let label = if let Some(el) = node.element_data() {
        let class = el
            .attrs
            .iter()
            .find(|a| *a.name.local == *"class")
            .map(|a| a.value.clone())
            .unwrap_or_default();
        format!("<{} class='{}'>", el.name.local, class)
    } else if let Some(text) = node.text_data() {
        let t: String = text.content.chars().take(24).collect();
        format!("text '{t}'")
    } else {
        "node".to_string()
    };
    let _ = writeln!(
        out,
        "{:indent$}#{id} {label} @ ({:.1},{:.1}) {:.1}x{:.1}",
        "",
        l.location.x,
        l.location.y,
        l.size.width,
        l.size.height,
        indent = depth * 2
    );
    for child in node.children.iter() {
        dump_layout(doc, *child, depth + 1, out);
    }
}

/// Dirty-reason bits — every `dirty = true` site tags WHY, so the perf log
/// can answer "what keeps invalidating the frame" without guessing.
mod dirty {
    pub const HEAD: u32 = 1;
    pub const POLL: u32 = 2;
    pub const ANIM: u32 = 4;
    pub const INPUT: u32 = 8;
    pub const RESIZE: u32 = 16;
    pub const INIT: u32 = 32;
    pub const RENDERER: u32 = 64;
    pub const EVENT: u32 = 128;
}

fn dirty_reasons_str(bits: u32) -> String {
    let mut names = Vec::new();
    for (bit, name) in [
        (dirty::HEAD, "head"),
        (dirty::POLL, "poll"),
        (dirty::ANIM, "anim"),
        (dirty::INPUT, "input"),
        (dirty::RESIZE, "resize"),
        (dirty::INIT, "init"),
        (dirty::RENDERER, "renderer"),
        (dirty::EVENT, "event"),
    ] {
        if bits & bit != 0 {
            names.push(name);
        }
    }
    names.join("+")
}

/// Monotonic milliseconds (page time origin) for phase timing.
fn perf_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// Accumulated frame-phase timings. Logs every rendered frame over the spike
/// threshold with a full breakdown, plus a periodic average so a steady-state
/// cost (e.g. "renders every frame because of a spinner") is visible without
/// flooding the console.
#[derive(Default)]
struct FramePerf {
    frames: u32,
    /// head / poll / resolve / scene+render / present — summed and maxed.
    sum: [f64; 5],
    max: [f64; 5],
    max_total: f64,
    reasons: u32,
}

impl FramePerf {
    const PHASES: [&'static str; 5] = ["head", "poll", "resolve", "render", "present"];
    const SPIKE_MS: f64 = 32.0;
    const REPORT_EVERY: u32 = 300;

    fn record(&mut self, phases: [f64; 5], reasons: u32) {
        self.frames += 1;
        self.reasons |= reasons;
        let mut total = 0.0;
        for (i, p) in phases.iter().enumerate() {
            self.sum[i] += p;
            if *p > self.max[i] {
                self.max[i] = *p;
            }
            total += p;
        }
        if total > self.max_total {
            self.max_total = total;
        }
        if total >= Self::SPIKE_MS {
            // info, not warn: devtools extensions (React's installHook) capture
            // a full wasm stack on every warn/error, which costs more main
            // thread time than the spike being reported.
            log::info!(
                target: NS,
                "perf spike frame={total:.1}ms head={:.1} poll={:.1} resolve={:.1} render={:.1} present={:.1} why={}",
                phases[0],
                phases[1],
                phases[2],
                phases[3],
                phases[4],
                dirty_reasons_str(reasons)
            );
        }
        if self.frames % Self::REPORT_EVERY == 0 {
            let n = self.frames as f64;
            log::info!(
                target: NS,
                "perf avg over {} frames: head={:.2} poll={:.2} resolve={:.2} render={:.2} present={:.2} (max_total={:.1}ms, why={})",
                self.frames,
                self.sum[0] / n,
                self.sum[1] / n,
                self.sum[2] / n,
                self.sum[3] / n,
                self.sum[4] / n,
                self.max_total,
                dirty_reasons_str(self.reasons)
            );
        }
    }
}

/// One animation frame. Returns false when the app was destroyed (the rAF
/// chain stops rescheduling itself).
fn frame(app_rc: &Rc<RefCell<WasmApp>>) -> bool {
    let t0 = perf_now();
    let mut app_mut = app_rc.borrow_mut();
    let app = &mut *app_mut;
    if !app.alive {
        return false;
    }

    // Head elements emitted by the last VDOM render (document::Style → CSS).
    while let Some(op) = app.head_ops.borrow_mut().pop_front() {
        app.doc
            .create_head_element(&op.name, &op.attributes, &op.contents);
        app.dirty = true;
        app.dirty_reasons |= dirty::HEAD;
    }
    let t1 = perf_now();

    // Flush VDOM work (signals poked by send_event / in-app events).
    if app.doc.poll(None) {
        app.dirty = true;
        app.dirty_reasons |= dirty::POLL;
    }
    if app.doc.is_animating() {
        app.dirty = true;
        app.dirty_reasons |= dirty::ANIM;
    }
    let t2 = perf_now();
    if !app.dirty {
        return true;
    }
    app.dirty = false;
    let reasons = core::mem::take(&mut app.dirty_reasons);

    let (RendererState::Pending | RendererState::Failed) = app.renderer else {
        let time_secs = perf_now() / 1000.0;
        app.doc.resolve(time_secs);
        let t3 = perf_now();

        if !app.layout_dumped {
            app.layout_dumped = true;
            let mut out = String::new();
            let root_id = app.doc.root_node().id;
            dump_layout(&app.doc, root_id, 0, &mut out);
            log::info!(target: NS, "layout dump\n{out}");
        }

        let (w, h, scale) = (app.width, app.height, app.scale as f64);
        let mut t4 = t3;
        let mut t5 = t3;
        match &mut app.renderer {
            RendererState::Active { surface } => {
                {
                    let mut painter = VelloScenePainter::new(&mut app.scene);
                    paint_scene(&mut painter, &app.doc, scale, w, h);
                }
                GPU_CACHE.with(|c| {
                    let mut cache = c.borrow_mut();
                    match cache.as_mut() {
                        Some(gpu) => {
                            if let Err(e) = gpu.vello.render_to_texture(
                                surface.device(),
                                surface.queue(),
                                &app.scene,
                                &surface.target_texture_view(),
                                &RenderParams {
                                    base_color: Color::WHITE,
                                    width: w,
                                    height: h,
                                    // Analytic AA: at 5M+ physical pixels MSAA16 costs more
                                    // than the scene itself; Area is vello's cheap path.
                                    antialiasing_method: AaConfig::Area,
                                },
                            ) {
                                log::error!(target: NS, "render_to_texture failed: {e:?}");
                            }
                        }
                        None => log::error!(target: NS, "GPU cache missing for active renderer"),
                    }
                });
                t4 = perf_now();
                surface.maybe_blit_and_present();
                t5 = perf_now();
                app.scene.reset();
            }
            RendererState::Cpu {
                renderer,
                ctx,
                buffer,
            } => {
                // White base (the GPU path's `base_color` equivalent).
                for px in buffer.iter_mut() {
                    *px = 255;
                }
                renderer.render(
                    |painter| paint_scene(painter, &app.doc, scale, w, h),
                    buffer,
                );
                match ImageData::new_with_u8_clamped_array_and_sh(
                    wasm_bindgen::Clamped(buffer.as_slice()),
                    w,
                    h,
                ) {
                    Ok(image) => {
                        let _ = ctx.put_image_data(&image, 0.0, 0.0);
                    }
                    Err(e) => log::error!(target: NS, "ImageData failed: {e:?}"),
                }
                t4 = perf_now();
                t5 = t4;
            }
            _ => {}
        }
        app.perf.record(
            [t1 - t0, t2 - t1, t3 - t2, t4 - t3, t5 - t4],
            reasons,
        );
        return true;
    };
    true
}

fn schedule_frame(raf: &Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>) {
    let Some(window) = web_sys::window() else { return };
    let borrow = raf.borrow();
    if let Some(cb) = borrow.as_ref() {
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
    }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn mods_from(e: &MouseEvent) -> Modifiers {
    let mut m = Modifiers::empty();
    if e.alt_key() {
        m |= Modifiers::ALT;
    }
    if e.shift_key() {
        m |= Modifiers::SHIFT;
    }
    if e.ctrl_key() {
        m |= Modifiers::CONTROL;
    }
    if e.meta_key() {
        m |= Modifiers::META;
    }
    m
}

fn button_from(e: &MouseEvent) -> MouseEventButton {
    match e.button() {
        1 => MouseEventButton::Auxiliary,
        2 => MouseEventButton::Secondary,
        3 => MouseEventButton::Fourth,
        4 => MouseEventButton::Fifth,
        _ => MouseEventButton::Main,
    }
}

enum MouseKind {
    Move,
    Down,
    Up,
}

fn mouse_closure(app_rc: &Rc<RefCell<WasmApp>>, kind: MouseKind) -> Closure<dyn FnMut(MouseEvent)> {
    let app_rc = app_rc.clone();
    Closure::wrap(Box::new(move |e: MouseEvent| {
        let ev = BlitzMouseButtonEvent {
            x: e.offset_x() as f32,
            y: e.offset_y() as f32,
            button: button_from(&e),
            buttons: MouseEventButtons::from_bits_truncate(e.buttons() as u8),
            mods: mods_from(&e),
        };
        let kind_name = match kind {
            MouseKind::Move => "move",
            MouseKind::Down => "down",
            MouseKind::Up => "up",
        };
        let (x, y) = (ev.x, ev.y);
        let ui = match kind {
            MouseKind::Move => UiEvent::MouseMove(ev),
            MouseKind::Down => UiEvent::MouseDown(ev),
            MouseKind::Up => UiEvent::MouseUp(ev),
        };
        let mut app = app_rc.borrow_mut();
        app.doc.handle_ui_event(ui);
        if !matches!(kind, MouseKind::Move) {
            let hit = app.doc.hit(x, y).map(|h| h.node_id);
            log::debug!(target: NS, "mouse {kind_name} x={x:.1} y={y:.1} hit={hit:?}");
        }
        app.dirty = true;
        app.dirty_reasons |= dirty::INPUT;
    }) as Box<dyn FnMut(MouseEvent)>)
}

fn resize_closure(app_rc: &Rc<RefCell<WasmApp>>) -> Closure<dyn FnMut()> {
    let app_rc = app_rc.clone();
    Closure::wrap(Box::new(move || {
        let mut app = app_rc.borrow_mut();
        let mut scale = web_sys::window()
            .map(|w| w.device_pixel_ratio() as f32)
            .unwrap_or(app.scale);
        if matches!(app.renderer, RendererState::Cpu { .. }) {
            scale = scale.min(CPU_DPR_CAP);
        }
        let css_w = app.canvas.client_width().max(1) as u32;
        let css_h = app.canvas.client_height().max(1) as u32;
        let w = (css_w as f32 * scale).round().max(1.0) as u32;
        let h = (css_h as f32 * scale).round().max(1.0) as u32;
        if w == app.width && h == app.height && scale == app.scale {
            return;
        }
        app.canvas.set_width(w);
        app.canvas.set_height(h);
        app.width = w;
        app.height = h;
        app.scale = scale;
        app.doc
            .set_viewport(Viewport::new(w, h, scale, ColorScheme::Light));
        match &mut app.renderer {
            RendererState::Active { surface, .. } => surface.resize(w, h),
            RendererState::Cpu {
                renderer, buffer, ..
            } => {
                renderer.resize(w, h);
                buffer.resize(w as usize * h as usize * 4, 255);
            }
            _ => {}
        }
        app.dirty = true;
        app.dirty_reasons |= dirty::RESIZE;
    }) as Box<dyn FnMut()>)
}

/// CPU rasterizer dpr ceiling. At dpr 2.5 the backing buffer is 6.25x the CSS
/// pixels — vello_cpu + a 21MB `put_image_data` per frame cannot hold 60fps at
/// that size, so the fallback renders at a capped ratio and lets the canvas
/// upscale slightly. The GPU path keeps full device resolution.
const CPU_DPR_CAP: f32 = 1.5;

/// Initialize the CPU rasterizer fallback after WebGPU acquisition failed.
/// The canvas wgpu probed is locked to the `webgpu` context type, so a fresh
/// canvas is swapped in and the mouse listeners re-registered on it.
fn init_cpu_renderer(app: &mut WasmApp) -> Result<(), String> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "create_element('canvas') failed")?
        .dyn_into()
        .map_err(|_| "created element is not a canvas")?;

    // Clamp the backing store to the dpr ceiling before sizing anything.
    let scale = web_sys::window()
        .map(|w| (w.device_pixel_ratio() as f32).min(CPU_DPR_CAP))
        .unwrap_or(app.scale.min(CPU_DPR_CAP));
    let css_w = app.canvas.client_width().max(1) as u32;
    let css_h = app.canvas.client_height().max(1) as u32;
    let w = (css_w as f32 * scale).round().max(1.0) as u32;
    let h = (css_h as f32 * scale).round().max(1.0) as u32;
    canvas.set_width(w);
    canvas.set_height(h);
    app.width = w;
    app.height = h;
    app.scale = scale;
    app.doc
        .set_viewport(Viewport::new(w, h, scale, ColorScheme::Light));
    let style = canvas.style();
    style
        .set_property("width", "100%")
        .map_err(|_| "style width failed")?;
    style
        .set_property("height", "100%")
        .map_err(|_| "style height failed")?;
    style
        .set_property("display", "block")
        .map_err(|_| "style display failed")?;

    let old = app.canvas.clone();
    let parent = old.parent_node().ok_or("old canvas has no parent")?;
    parent
        .replace_child(&canvas, &old)
        .map_err(|_| "canvas swap failed")?;

    if let Some(listeners) = &app.listeners {
        canvas
            .add_event_listener_with_callback(
                "mousemove",
                listeners.on_mouse_move.as_ref().unchecked_ref(),
            )
            .map_err(|_| "listener mousemove failed")?;
        canvas
            .add_event_listener_with_callback(
                "mousedown",
                listeners.on_mouse_down.as_ref().unchecked_ref(),
            )
            .map_err(|_| "listener mousedown failed")?;
        canvas
            .add_event_listener_with_callback("mouseup", listeners.on_mouse_up.as_ref().unchecked_ref())
            .map_err(|_| "listener mouseup failed")?;
    }

    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|_| "getContext('2d') threw")?
        .ok_or("getContext('2d') returned null")?
        .dyn_into()
        .map_err(|_| "2d context has wrong type")?;
    let renderer = VelloCpuImageRenderer::new(app.width, app.height);
    let buffer = vec![255u8; app.width as usize * app.height as usize * 4];
    app.canvas = canvas;
    app.renderer = RendererState::Cpu {
        renderer,
        ctx,
        buffer,
    };
    log::info!(
        target: NS,
        "renderer active (vello_cpu → canvas 2d, no WebGPU) size={}x{}@{} (dpr capped at {})",
        app.width,
        app.height,
        app.scale,
        CPU_DPR_CAP
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// JS bridge
// ---------------------------------------------------------------------------

fn js_err(e: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn element_by_id(element_id: &str) -> Result<web_sys::Element, JsValue> {
    let id = element_id.strip_prefix('#').unwrap_or(element_id);
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("element '#{id}' not found")))
}

fn teardown(app_rc: &Rc<RefCell<WasmApp>>) {
    let mut app = app_rc.borrow_mut();
    app.alive = false;
    app.canvas.remove();
}

/// Installs the language data (pattern vocabulary, theme presets, operator
/// table) before the first `start`: a browser has no `node_modules` to read, so
/// the page hands over a bundle assembled from ITS installed `@almadar/core` and
/// `@almadar/std` (`orbital_core::language_data::install_language_bundle`).
#[wasm_bindgen]
pub fn install_language(bundle_json: String) -> Result<(), JsValue> {
    orbital_core::language_data::install_language_bundle(&bundle_json)
        .map_err(|e| js_err(format!("{}: {e}", e.code())))
}

/// Handle to a running Orbital Dioxus-Blitz app. Created from JS as
/// `new WebHandle()`. Same surface the egui wasm shell exposes.
#[wasm_bindgen]
pub struct WebHandle {
    _private: (),
}

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        LOG_INIT.call_once(|| {
            let _ = log::set_logger(&FILTERING_LOGGER);
            // Runtime log level: `?log=warn|info|debug` URL param, default
            // info. Every console line crosses the wasm→JS boundary and is
            // rendered on the main thread when devtools is open — a mount's
            // ~40 info lines measurably cost hundreds of ms there, so the
            // level is switchable without a rebuild (also `set_log_level`).
            let level = web_sys::window()
                .and_then(|w| w.location().search().ok())
                .and_then(|q| {
                    q.trim_start_matches('?')
                        .split('&')
                        .find_map(|kv| kv.strip_prefix("log=").map(str::to_string))
                })
                .and_then(|v| v.parse::<log::LevelFilter>().ok())
                .unwrap_or(log::LevelFilter::Info);
            log::set_max_level(level);
            let _ = tracing::subscriber::set_global_default(ConsoleSubscriber);
        });
        log::info!(target: NS, "WebHandle (dioxus-native/blitz) created (log @ {:?}, stylo/blitz quiet)", log::max_level());
        Self { _private: () }
    }

    /// Switch the runtime log level without a rebuild
    /// (`off|error|warn|info|debug|trace`). Unknown values are ignored.
    #[wasm_bindgen]
    pub fn set_log_level(&self, level: &str) {
        if let Ok(l) = level.parse::<log::LevelFilter>() {
            log::set_max_level(l);
            log::info!(target: NS, "log level -> {:?}", l);
        }
    }

    /// Mount the app into `<element id="element_id">` from an `.orb` JSON
    /// schema. A `<canvas>` is created inside the element and Blitz paints
    /// into it (WebGPU). `css_text` is stylesheet content (almadar.css +
    /// tailwind.css concatenated by the TS bridge) — Stylo resolves every
    /// `class:` attribute against it, same as the desktop `--css` path.
    #[wasm_bindgen]
    pub fn start(
        &self,
        element_id: String,
        schema_json: String,
        css_text: String,
    ) -> Result<(), JsValue> {
        let t_s0 = perf_now();
        let (orbitals, traits) = shell::mount_core(&schema_json).map_err(js_err)?;
        let t_s1 = perf_now();
        let css_bytes = css_text.len();

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let host = element_by_id(&element_id)?;
        let scale = window.device_pixel_ratio() as f32;
        let css_w = host.client_width().max(1) as u32;
        let css_h = host.client_height().max(1) as u32;
        let width = (css_w as f32 * scale).round().max(1.0) as u32;
        let height = (css_h as f32 * scale).round().max(1.0) as u32;

        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| JsValue::from_str("create_element('canvas') failed"))?
            .dyn_into()
            .map_err(|_| JsValue::from_str("created element is not a canvas"))?;
        canvas.set_width(width);
        canvas.set_height(height);
        let style = canvas.style();
        style
            .set_property("width", "100%")
            .map_err(|_| JsValue::from_str("style width failed"))?;
        style
            .set_property("height", "100%")
            .map_err(|_| JsValue::from_str("style height failed"))?;
        style
            .set_property("display", "block")
            .map_err(|_| JsValue::from_str("style display failed"))?;
        host.append_child(&canvas)
            .map_err(|_| JsValue::from_str("append canvas failed"))?;

        CSS_TEXT.with(|c| {
            *c.borrow_mut() = if css_text.trim().is_empty() {
                Vec::new()
            } else {
                vec![css_text]
            }
        });

        // Build the VDOM with the head-element sink, then the Blitz document —
        // the same sequence as dioxus-native's `resumed`.
        let head_ops: Rc<RefCell<VecDeque<crate::offscreen::HeadOp>>> = Rc::new(RefCell::new(VecDeque::new()));
        let vdom = VirtualDom::new(App);
        let sink = crate::offscreen::HeadSink {
            ops: head_ops.clone(),
        };
        vdom.in_scope(ScopeId::ROOT, || {
            provide_context(Rc::new(sink) as Rc<dyn dioxus_document::Document>);
        });
        let mut doc = DioxusDocument::new(
            vdom,
            DocumentConfig {
                viewport: Some(Viewport::new(width, height, scale, ColorScheme::Light)),
                font_ctx: Some(crate::offscreen::embedded_font_context()),
                ..Default::default()
            },
        );
        let t_s2 = perf_now();
        doc.initial_build();
        let t_s3 = perf_now();
        log::info!(
            target: NS,
            "start timing mount_core={:.0}ms doc_new={:.0}ms initial_build={:.0}ms",
            t_s1 - t_s0,
            t_s2 - t_s1,
            t_s3 - t_s2
        );

        let app_rc = Rc::new(RefCell::new(WasmApp {
            doc,
            scene: Scene::new(),
            renderer: RendererState::Pending,
            canvas: canvas.clone(),
            width,
            height,
            scale,
            dirty: true,
            dirty_reasons: dirty::INIT,
            perf: FramePerf::default(),
            alive: true,
            layout_dumped: false,
            head_ops,
            listeners: None,
            raf: Rc::new(RefCell::new(None)),
        }));

        // wgpu surface: async (WebGPU promises) — block_on would deadlock.
        // The GPU context + vello renderer are process-wide (GPU_CACHE); only
        // the surface is per-mount. If a previous mount proved wgpu unusable,
        // skip the whole attempt and go straight to the CPU rasterizer.
        let gpu_unavailable = GPU_UNAVAILABLE.with(|c| c.get());
        if gpu_unavailable {
            let mut app = app_rc.borrow_mut();
            match init_cpu_renderer(&mut app) {
                Ok(()) => {
                    app.dirty = true;
                    app.dirty_reasons |= dirty::RENDERER;
                    log::info!(target: NS, "wgpu cached-unavailable — CPU rasterizer directly");
                }
                Err(msg) => {
                    app.renderer = RendererState::Failed;
                    log::error!(target: NS, "vello_cpu fallback failed: {msg}");
                }
            }
        } else {
            let surface_cfg = SurfaceRendererConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                formats: vec![
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureFormat::Bgra8Unorm,
                ],
                width,
                height,
                present_mode: wgpu::PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            };
            let tex_cfg = TextureConfiguration {
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            };
            wasm_bindgen_futures::spawn_local({
                let app_rc = app_rc.clone();
                let canvas = canvas.clone();
                async move {
                    // Take the shared GPU context out for the await (a RefCell
                    // borrow can't span it). A concurrent mount in this window
                    // simply builds its own and stores back — last writer wins,
                    // both vello renderers share the same device either way.
                    let taken = GPU_CACHE.with(|c| c.borrow_mut().take());
                    let (mut wgpu_ctx, cached_vello) = match taken {
                        Some(g) => (g.wgpu_ctx, Some(g.vello)),
                        None => (WGPUContext::new(), None),
                    };
                    let vello_reused = cached_vello.is_some();
                    let t_gpu0 = perf_now();
                    let result = wgpu_ctx
                        .create_surface(SurfaceTarget::Canvas(canvas), surface_cfg, Some(tex_cfg))
                        .await;
                    let surface_ms = perf_now() - t_gpu0;
                    let mut app = app_rc.borrow_mut();
                    // The watchdog may have already installed the CPU renderer
                    // while this promise was in flight — never retrograde.
                    if !matches!(app.renderer, RendererState::Pending) {
                        if let Some(vello) = cached_vello {
                            GPU_CACHE.with(|c| {
                                *c.borrow_mut() = Some(GpuCache { wgpu_ctx, vello });
                            });
                        }
                        log::info!(target: NS, "wgpu resolved after watchdog fired — keeping CPU renderer");
                        return;
                    }
                    match result {
                        Ok(surface) => {
                            let vello = match cached_vello {
                                Some(v) => Some(v),
                                None => {
                                    let t_v = perf_now();
                                    match VelloRenderer::new(
                                        surface.device(),
                                        RendererOptions {
                                            // We only ever render with
                                            // AaConfig::Area — compiling every
                                            // AA pipeline permutation
                                            // (AaSupport::all()) wastes seconds
                                            // of driver shader compilation on
                                            // the one-time vello init.
                                            antialiasing_support: AaSupport::area_only(),
                                            use_cpu: false,
                                            num_init_threads: std::num::NonZeroUsize::new(1),
                                            pipeline_cache: None,
                                        },
                                    ) {
                                        Ok(v) => {
                                            log::info!(
                                                target: NS,
                                                "vello renderer created surface={surface_ms:.0}ms vello_init={:.0}ms",
                                                perf_now() - t_v
                                            );
                                            Some(v)
                                        }
                                        Err(e) => {
                                            log::warn!(target: NS, "vello renderer init failed ({e:?}) — CPU fallback (cached: future mounts skip wgpu)");
                                            GPU_UNAVAILABLE.with(|c| c.set(true));
                                            None
                                        }
                                    }
                                }
                            };
                            match vello {
                                Some(vello) => {
                                    GPU_CACHE.with(|c| {
                                        *c.borrow_mut() = Some(GpuCache { wgpu_ctx, vello });
                                    });
                                    app.renderer = RendererState::Active { surface };
                                    app.dirty = true;
                                    app.dirty_reasons |= dirty::RENDERER;
                                    log::info!(
                                        target: NS,
                                        "renderer active (vello/wgpu → canvas) surface={surface_ms:.0}ms reused={vello_reused}"
                                    );
                                }
                                None => {
                                    match init_cpu_renderer(&mut app) {
                                        Ok(()) => {
                                            app.dirty = true;
                                            app.dirty_reasons |= dirty::RENDERER;
                                        }
                                        Err(msg) => {
                                            app.renderer = RendererState::Failed;
                                            log::error!(target: NS, "vello_cpu fallback failed: {msg}");
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            GPU_UNAVAILABLE.with(|c| c.set(true));
                            log::warn!(target: NS, "WebGPU unavailable ({e}) — CPU fallback (cached: future mounts skip wgpu)");
                            match init_cpu_renderer(&mut app) {
                                Ok(()) => {
                                    app.dirty = true;
                                    app.dirty_reasons |= dirty::RENDERER;
                                }
                                Err(msg) => {
                                    app.renderer = RendererState::Failed;
                                    log::error!(target: NS, "vello_cpu fallback failed: {msg}");
                                }
                            }
                        }
                    }
                }
            });

            // Watchdog: if the WebGPU promise neither resolves nor rejects within
            // a few seconds (real browsers can stall inside requestAdapter), drop
            // to the CPU rasterizer so the app never sits on a blank canvas.
            {
                let app_for_watchdog = app_rc.clone();
                let watchdog = Closure::wrap(Box::new(move || {
                    let mut app = app_for_watchdog.borrow_mut();
                    if matches!(app.renderer, RendererState::Pending) && app.alive {
                        log::warn!(target: NS, "WebGPU init timed out — falling back to vello_cpu rasterizer");
                        match init_cpu_renderer(&mut app) {
                            Ok(()) => {
                                app.dirty = true;
                                app.dirty_reasons |= dirty::RENDERER;
                            }
                            Err(msg) => {
                                app.renderer = RendererState::Failed;
                                log::error!(target: NS, "vello_cpu fallback failed: {msg}");
                            }
                        }
                    }
                }) as Box<dyn FnMut()>);
                window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        watchdog.as_ref().unchecked_ref(),
                        8000,
                    )
                    .map_err(|_| JsValue::from_str("watchdog setTimeout failed"))?;
                watchdog.forget();
            }
        }

        // Input listeners.
        let on_mouse_move = mouse_closure(&app_rc, MouseKind::Move);
        let on_mouse_down = mouse_closure(&app_rc, MouseKind::Down);
        let on_mouse_up = mouse_closure(&app_rc, MouseKind::Up);
        canvas
            .add_event_listener_with_callback("mousemove", on_mouse_move.as_ref().unchecked_ref())
            .map_err(|_| JsValue::from_str("listener mousemove failed"))?;
        canvas
            .add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref())
            .map_err(|_| JsValue::from_str("listener mousedown failed"))?;
        canvas
            .add_event_listener_with_callback("mouseup", on_mouse_up.as_ref().unchecked_ref())
            .map_err(|_| JsValue::from_str("listener mouseup failed"))?;
        let on_resize = resize_closure(&app_rc);
        window
            .add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref())
            .map_err(|_| JsValue::from_str("listener resize failed"))?;

        // rAF loop: the closure re-schedules itself while the app is alive.
        let raf_cell: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
        {
            let raf_for_closure = raf_cell.clone();
            let app_for_frame = app_rc.clone();
            *raf_cell.borrow_mut() = Some(Closure::wrap(Box::new(move |_t: f64| {
                if frame(&app_for_frame) {
                    schedule_frame(&raf_for_closure);
                }
            }) as Box<dyn FnMut(f64)>));
        }
        schedule_frame(&raf_cell);

        // Keep listeners + raf alive inside the app state.
        {
            let mut app = app_rc.borrow_mut();
            app.listeners = Some(Listeners {
                on_mouse_move,
                on_mouse_down,
                on_mouse_up,
                on_resize,
            });
            app.raf = raf_cell;
        }

        WASM_APP.with(|cell| {
            if let Some(old) = cell.borrow_mut().replace(app_rc) {
                teardown(&old);
            }
        });

        log::info!(
            target: NS,
            "start: canvas={}x{}@{} orbitals={} traits={} schema_bytes={} css_bytes={} state='{}'",
            width,
            height,
            scale,
            orbitals,
            traits,
            schema_json.len(),
            css_bytes,
            shell::current_state_string().unwrap_or_default(),
        );
        Ok(())
    }

    /// Drive one event synchronously from JS (e.g. the verify harness). Empty /
    /// `null` payload is allowed. Pokes the root component to re-render.
    #[wasm_bindgen]
    pub fn send_event(&self, event: String, payload_json: String) -> Result<(), JsValue> {
        shell::drive_event(&event, &payload_json).map_err(js_err)?;
        poke_rerender();
        with_app(|app| {
            app.dirty = true;
            app.dirty_reasons |= dirty::EVENT;
        });
        Ok(())
    }

    /// Snapshot current render state as JSON for the TS bridge.
    #[wasm_bindgen]
    pub fn snapshot(&self) -> Result<String, JsValue> {
        shell::snapshot_json().map_err(js_err)
    }

    /// Current active state-machine state.
    #[wasm_bindgen]
    pub fn current_state(&self) -> Result<String, JsValue> {
        shell::current_state_string().map_err(js_err)
    }

    /// Unmount: stop the rAF chain, remove the canvas, drop the app state.
    #[wasm_bindgen]
    pub fn destroy(&self) {
        WASM_APP.with(|cell| {
            if let Some(old) = cell.borrow_mut().take() {
                teardown(&old);
            }
        });
        shell::unmount();
        log::info!(target: NS, "destroy: app unmounted");
    }
}

/// Run the same verify building blocks `orb verify` does — L1 (the kernel walk,
/// `orbital_core::verify`) and L2 (the render contract,
/// `orbital_core::client::render_probe`, against this renderer) — on a program
/// `orb` resolved, from inside the browser. Stateless (no canvas). Returns JSON:
/// `{ item, ok, l1: { ok, headline, summary, verdicts[] }, l2: { ok, headline,
///   slots_rendered, stubbed, verdicts[] } }`; every verdict is `{ name, passed, detail }`.
#[wasm_bindgen]
pub fn verify_schema_json(resolved_json: String) -> Result<String, JsValue> {
    let schema = orbital_core::schema::parse_schema(&resolved_json).map_err(js_err)?;
    let item = schema.name.clone();
    log::info!(target: NS, "verify: item='{}' orbitals={} schema_bytes={}", item, schema.orbitals.len(), resolved_json.len());

    let l1 = orbital_core::verify::verify_schema(&item, schema.clone()).map_err(js_err)?;
    log::info!(target: NS, "verify L1: {}", l1.headline());
    for v in &l1.verdicts {
        log::info!(target: NS, "verify L1 verdict={} passed={} detail='{}'", v.name, v.passed, v.detail);
    }

    let target = crate::probe_target::DioxusTarget::new();
    let l2 = orbital_core::client::render_probe::probe(&item, schema, None, &target).map_err(js_err)?;
    log::info!(target: NS, "verify L2: {}", l2.headline());
    for v in &l2.verdicts {
        log::info!(target: NS, "verify L2 verdict={} passed={} detail='{}'", v.name, v.passed, v.detail);
    }

    let ok = l1.ok() && l2.ok();
    log::info!(target: NS, "verify done: item='{}' ok={}", item, ok);
    let out = serde_json::json!({
        "item": item,
        "ok": ok,
        "l1": { "ok": l1.ok(), "headline": l1.headline(), "summary": l1.summary, "verdicts": l1.verdicts },
        "l2": { "ok": l2.ok(), "headline": l2.headline(), "slots_rendered": l2.slots_rendered, "stubbed": l2.stubbed, "verdicts": l2.verdicts },
    });
    serde_json::to_string(&out).map_err(js_err)
}
