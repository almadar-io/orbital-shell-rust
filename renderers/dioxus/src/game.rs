//! Scene pattern renderers for the native (Blitz) shell — every Rust-drawn scene
//! (`scene_patterns`: learning canvases, diagrams, `svg-*` atoms, the game `canvas`) and `game-hud`.
//!
//! The scene patterns paint through an anyrender_vello [`CustomPaintSource`]
//! that SHARES the window's wgpu device: props → display-list packet via
//! `orbital_canvas::SceneHost` (the ONE scene host the Dioxus and mobile
//! shells drive: mapping, layout memory, taps, clock) → [`SharedDeviceRenderer`] into
//! an offscreen texture → composited into the Blitz scene by vello
//! (`register_texture`). Blitz marks a document with a canvas paint source as
//! animating (`blitz-dom mutator.rs` sets `recompute_is_animating` on
//! `canvas[src=<u64>]`), so the canvas repaints every vsync with no paint-side
//! tick infrastructure. State-machine TICKS are driven separately by the frame
//! hook installed in `native::start` (see `shell::drive_ticks`).

#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use crate::helpers::{format_value, get_entity_field, get_hud_stats, humanize_field_name};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use anyrender_vello::wgpu;
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use dioxus::prelude::*;
use dioxus_native::use_wgpu;
use orbital_canvas::mappers::{learning, scene};
use orbital_canvas::{SceneHost, SharedDeviceRenderer};
use orbital_core::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use web_time::Instant;

/// Log namespace — mirrors `@almadar/logger`'s `almadar:ui-native-game`.
const NS: &str = "almadar::ui_native_game";

/// The last scene node each pattern converted, with the kernel value and theme it came from.
type Converted = (Value, Option<serde_json::Value>, Arc<serde_json::Value>);

thread_local! {
    static CONVERTED: std::cell::RefCell<std::collections::HashMap<String, Converted>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Whether `a` and `b` are one kernel allocation: a scene the kernel did not re-render is handed over unchanged.
fn same_allocation(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => orbital_core::Shared::ptr_eq(a, b),
        _ => false,
    }
}

/// The node a scene pattern's host draws: its props under its `type`, plus the kernel's theme. Every dispatch renders
/// the page again, so a scene the kernel handed over unchanged reuses the node it was converted to.
fn scene_node(
    pattern: &str,
    props: &Value,
    canvas_theme: Option<&serde_json::Value>,
) -> Arc<serde_json::Value> {
    let reused = CONVERTED.with(|c| {
        c.borrow()
            .get(pattern)
            .filter(|(from, theme, _)| same_allocation(from, props) && theme.as_ref() == canvas_theme)
            .map(|(_, _, node)| Arc::clone(node))
    });
    if let Some(node) = reused {
        return node;
    }
    let node = Arc::new(scene::host_node(
        pattern,
        serde_json::to_value(props).unwrap_or_default(),
        canvas_theme,
    ));
    CONVERTED.with(|c| {
        c.borrow_mut()
            .insert(pattern.to_string(), (props.clone(), canvas_theme.cloned(), Arc::clone(&node)))
    });
    node
}

/// The canvas element's size in CSS px: the scene's own size. A scene the native painter refuses is logged and
/// takes no room.
fn canvas_size(node: &serde_json::Value) -> Option<(usize, usize)> {
    match scene::scene_size(node) {
        Ok((w, h)) => Some((w.max(1.0) as usize, h.max(1.0) as usize)),
        Err(err) => {
            tracing::warn!(target: NS, error = %err, "scene refused");
            None
        }
    }
}

/// Whether the scene is an orbital-painter canvas that fills its parent (`fillContainer: true`)
/// instead of taking its own size.
fn fills_container(node: &serde_json::Value) -> bool {
    let fields = node.get("props").unwrap_or(node);
    fields.get("painter").and_then(serde_json::Value::as_str) == Some("orbital")
        && fields.get("fillContainer").and_then(serde_json::Value::as_bool) == Some(true)
}

/// The least height in px of a canvas that fills its parent (`minHeight`; 0 when none is declared).
fn min_height(node: &serde_json::Value) -> f64 {
    let fields = node.get("props").unwrap_or(node);
    fields.get("minHeight").and_then(serde_json::Value::as_f64).filter(|h| h.is_finite() && *h > 0.0).unwrap_or(0.0)
}

/// Whether the scene is an orbital-painter canvas that takes its parent's width and its own
/// height (`fillContainer: "width"`), so the page scrolls past a long scene.
fn fills_width(node: &serde_json::Value) -> bool {
    let fields = node.get("props").unwrap_or(node);
    fields.get("painter").and_then(serde_json::Value::as_str) == Some("orbital")
        && fields.get("fillContainer").and_then(serde_json::Value::as_str) == Some("width")
}

/// Paints one scene into the Blitz scene via the shared wgpu device. The scene host is shared
/// with the component, which feeds it taps; this side keeps its viewport, clock, assets and fonts
/// current and draws its frame. Owns the offscreen target texture (recreated on size change).
/// The scene node the component last rendered, versioned so the paint source maps it only when it changed.
struct SceneNode {
    version: u64,
    node: Arc<serde_json::Value>,
}

/// Each mounted scene canvas and whether it has drawn a frame its program marked `ready`: the web's `data-orb-ready`,
/// which an `untilReady` loading state waits on.
thread_local! {
    static SCENES_READY: std::cell::RefCell<Vec<(u64, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The signals of the mounted boot loaders, bumped when readiness changes so each one renders again.
    static READY_WATCHERS: std::cell::RefCell<Vec<Signal<u64>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Re-render `watcher`'s component whenever scene readiness changes, until [`unwatch_scenes_ready`].
pub(crate) fn watch_scenes_ready(watcher: Signal<u64>) {
    READY_WATCHERS.with(|w| w.borrow_mut().push(watcher));
}

pub(crate) fn unwatch_scenes_ready(watcher: Signal<u64>) {
    READY_WATCHERS.with(|w| w.borrow_mut().retain(|s| *s != watcher));
}
static NEXT_SCENE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether at least one scene canvas is mounted and every one has drawn a ready frame.
pub(crate) fn scenes_ready() -> bool {
    SCENES_READY.with(|scenes| {
        let scenes = scenes.borrow();
        !scenes.is_empty() && scenes.iter().all(|(_, ready)| *ready)
    })
}

fn set_scene_ready(key: u64, ready: Option<bool>) {
    let changed = SCENES_READY.with(|scenes| {
        let mut scenes = scenes.borrow_mut();
        match (scenes.iter().position(|(k, _)| *k == key), ready) {
            (Some(i), None) => {
                scenes.remove(i);
                true
            }
            (Some(i), Some(ready)) => std::mem::replace(&mut scenes[i].1, ready) != ready,
            // Registering during the component's render pokes nothing: a new canvas is not ready, which is what any
            // loader reading the list this pass sees.
            (None, Some(ready)) => {
                scenes.push((key, ready));
                false
            }
            (None, None) => false,
        }
    });
    if changed {
        READY_WATCHERS.with(|w| {
            for watcher in w.borrow_mut().iter_mut() {
                let next = watcher.peek().wrapping_add(1);
                watcher.set(next);
            }
        });
    }
}

/// Whether the canvas box Blitz hands the paint source has been laid out to an area.
fn box_has_area(width: u32, height: u32) -> bool {
    width > 0 && height > 0
}

/// A frame counts as the program's laid-out scene once it was drawn with `ready` and nothing it asked for (a font, an
/// image) is still downloading: the web draws its first ready frame with the page's fonts already in the browser,
/// while a native host fetches them at run time and lays the scene out again when they land. A frame that queued
/// events the program has not received yet (the sizes it just measured) is drawn before the program lays out again.
fn drawn_ready(marked: bool, assets_pending: usize, unsent: bool) -> bool {
    marked && assets_pending == 0 && !unsent
}

/// The program's `ready` on a scene node: its scene is laid out.
fn marked_ready(node: &serde_json::Value) -> bool {
    node.get("props").unwrap_or(node).get("ready").and_then(serde_json::Value::as_bool) == Some(true)
}

struct GamePaintSource {
    node: Arc<Mutex<SceneNode>>,
    /// This canvas's entry in the scene readiness list.
    scene_key: u64,
    /// The node version the scene host holds.
    mapped: Option<u64>,
    host: Arc<Mutex<SceneHost>>,
    renderer: Option<SharedDeviceRenderer>,
    target: Option<(wgpu::Texture, TextureHandle, u32, u32)>,
    started: Instant,
    renders: u32,
}

impl GamePaintSource {
    fn new(node: Arc<Mutex<SceneNode>>, scene_key: u64, host: Arc<Mutex<SceneHost>>) -> Self {
        Self {
            node,
            scene_key,
            mapped: None,
            host,
            renderer: None,
            target: None,
            started: Instant::now(),
            renders: 0,
        }
    }
}

impl CustomPaintSource for GamePaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        let device = Arc::new(device_handle.device.clone());
        let queue = Arc::new(device_handle.queue.clone());
        if let Err(error) = orbital_canvas::register_device(device.clone(), queue.clone()) {
            tracing::debug!(target: NS, %error, "window device not registered; the slot keeps its device");
        }
        self.renderer = Some(SharedDeviceRenderer::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        tracing::info!(target: NS, "game paint source resumed (shared wgpu device)");
    }

    fn suspend(&mut self) {
        self.renderer = None;
        self.target = None;
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        // As the web canvas, which reports its first size once its box is laid out: a box with no area yet is not
        // drawn, so the program never lays its scene out at a provisional size and calls that laid out (ready).
        if !box_has_area(width, height) {
            return None;
        }
        let renderer = self.renderer.as_mut()?;
        // Blitz reports the canvas content box ALREADY in physical px (CSS px
        // × dpr — proven: `game paint render css=2000x1500` for an 800x600
        // logical canvas at 2.5 dpr). Render 1:1 at that size; vello stretches
        // the registered texture back over the box (`scene.rs`
        // `render_custom_source`). Do NOT multiply by `scale` again — that
        // double-scales the target (5000x3750 = 6.25× the needed pixels).
        // Within the device's texture limit, as orbital-canvas sizes its own targets: the same logical box, at the
        // density that fits (a phone-tall canvas would otherwise exceed a 2048 limit and the frame paints nothing).
        let points = (
            (f64::from(width.max(1)) / scale).round().max(1.0) as u32,
            (f64::from(height.max(1)) / scale).round().max(1.0) as u32,
        );
        let limit = renderer.device().limits().max_texture_dimension_2d;
        let density = orbital_canvas::render_target::density_within(points, scale as f32, limit);
        let (phys_w, phys_h) = if density < scale as f32 {
            (
                ((points.0 as f32 * density).round().max(1.0) as u32).min(limit),
                ((points.1 as f32 * density).round().max(1.0) as u32).min(limit),
            )
        } else {
            (width.max(1), height.max(1))
        };

        let stale = self
            .target
            .as_ref()
            .is_none_or(|(_, _, w, h)| *w != phys_w || *h != phys_h);
        if stale {
            if let Some((_, handle, _, _)) = self.target.take() {
                ctx.unregister_texture(handle);
            }
            let texture = renderer.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("canvas-2d target"),
                size: wgpu::Extent3d {
                    width: phys_w,
                    height: phys_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                // COPY_SRC is mandatory: vello copies registered textures into
                // its image atlas each frame (vello::Renderer::register_texture
                // docs); without it vello's encoder fails validation at finish.
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let handle = ctx.register_texture(texture.clone());
            self.target = Some((texture, handle, phys_w, phys_h));
        }

        let (packet, unsent) = {
            let mut host = self.host.lock().expect("scene host poisoned");
            if host.density() != density && host.set_scale(density).is_err() {
                return None;
            }
            if host.viewport() != Some((phys_w, phys_h)) {
                host.set_viewport(phys_w, phys_h);
            }
            #[cfg(not(target_arch = "wasm32"))]
            let fonts_landed = orbital_canvas::system_fonts::supply_missing_from_system() > 0;
            #[cfg(target_arch = "wasm32")]
            let fonts_landed = false;
            // A frame maps the node only when the component handed over a new one (or fonts landed): cloning and
            // comparing the whole scene every frame cost 3.5 ms of a phone frame.
            let fresh = {
                let cell = self.node.lock().expect("scene node poisoned");
                (self.mapped != Some(cell.version) || fonts_landed).then(|| (cell.version, cell.node.clone()))
            };
            if let Some((version, node)) = fresh {
                self.mapped = Some(version);
                if let Err(err) = host.set_scene(node) {
                    tracing::warn!(target: NS, error = %err, "scene refused");
                    return None;
                }
            }
            host.refresh_assets();
            host.set_clock(self.started.elapsed().as_secs_f64() * 1000.0);
            if let Ok(epoch) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                host.set_epoch_clock(epoch.as_secs_f64() * 1000.0);
            }
            (host.frame_packet()?, host.has_scene_events())
        };
        let (texture, handle, _, _) = self.target.as_ref()?;
        let ok = renderer.render_packet_to_texture(&packet, texture);
        if ok {
            let marked = marked_ready(&self.node.lock().expect("scene node poisoned").node);
            set_scene_ready(self.scene_key, Some(drawn_ready(marked, orbital_canvas::asset::pending(), unsent)));
        }
        self.renders += 1;
        if stale || self.renders <= 3 || self.renders % 600 == 0 {
            tracing::info!(
                target: NS,
                n = self.renders,
                phys = %format_args!("{width}x{height}"),
                texture = %format_args!("{phys_w}x{phys_h}"),
                limit,
                commands = packet.draw_commands.len(),
                ok,
                "game paint render"
            );
        }
        if ok {
            Some(handle.clone())
        } else {
            None
        }
    }
}

/// One Rust-drawn scene pattern: the canvas element sized to the scene, painted through the
/// shared wgpu device, and a tap routed through the scene host to the trait as an event.
#[derive(Props, Clone, PartialEq)]
struct SceneCanvasProps {
    node: Arc<serde_json::Value>,
    sink: Option<crate::renderer::EventSink>,
    fill_parent: bool,
    #[props(default)]
    fill_width: bool,
    #[props(default)]
    min_height: f64,
    a11y: SceneA11y,
    class_name: String,
    inline_style: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct SceneA11y {
    label: Option<String>,
    labelledby: Option<String>,
    describedby: Option<String>,
    live: Option<String>,
    current: Option<String>,
    pressed: Option<String>,
    expanded: Option<String>,
    selected: Option<String>,
    hidden: Option<String>,
    busy: Option<String>,
    role: Option<String>,
    lang: Option<String>,
    dir: Option<String>,
    tabindex: Option<String>,
}

impl SceneA11y {
    fn from_props(props: &Value) -> Self {
        let get = |key| {
            props
                .as_object()
                .and_then(|fields| fields.get(key))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let boolean = |key| {
            props
                .as_object()
                .and_then(|fields| fields.get(key))
                .and_then(Value::as_bool)
                .map(|value| value.to_string())
        };
        Self {
            label: get("aria-label"),
            labelledby: get("aria-labelledby"),
            describedby: get("aria-describedby"),
            live: get("aria-live"),
            current: get("aria-current"),
            pressed: get("aria-pressed"),
            expanded: boolean("aria-expanded"),
            selected: boolean("aria-selected"),
            hidden: boolean("aria-hidden"),
            busy: boolean("aria-busy"),
            role: get("role"),
            lang: get("lang"),
            dir: get("dir"),
            tabindex: props
                .as_object()
                .and_then(|fields| fields.get("tabIndex"))
                .and_then(Value::as_number)
                .map(|value| value.to_string()),
        }
    }
}

fn scene_presentation(props: &Value) -> (String, SceneA11y) {
    (
        merge_class_name("almadar-game-canvas", props),
        SceneA11y::from_props(props),
    )
}

#[allow(non_snake_case)] // dioxus component convention (PascalCase)
fn SceneCanvas(props: SceneCanvasProps) -> Element {
    let node_cell = use_hook(|| Arc::new(Mutex::new(SceneNode { version: 0, node: props.node.clone() })));
    let host = use_hook(|| Arc::new(Mutex::new(SceneHost::new())));
    let held = use_hook(|| Arc::new(AtomicUsize::new(0)));
    // A vdom render hands over a changed node under a new version; the paint source re-maps on a new version only.
    {
        let mut cell = node_cell.lock().expect("scene node poisoned");
        if cell.node != props.node {
            cell.node = props.node.clone();
            cell.version += 1;
        }
    }
    let scene_key = use_hook(|| {
        let key = NEXT_SCENE.fetch_add(1, Ordering::Relaxed);
        set_scene_ready(key, Some(false));
        key
    });
    let id = use_wgpu({
        let (node_cell, host) = (node_cell.clone(), host.clone());
        move || GamePaintSource::new(node_cell, scene_key, host)
    });
    use_hook({
        let host = host.clone();
        move || {
            blitz_traits::canvas_access::register_canvas_access(
                id,
                Arc::new(crate::canvas_access::SceneAccess::new(host)),
            )
        }
    });
    use_drop(move || blitz_traits::canvas_access::unregister_canvas_access(id));
    use_drop(move || set_scene_ready(scene_key, None));
    let (w, h) = if props.fill_parent {
        (1, 1)
    } else if props.fill_width {
        let Some((_, h)) = canvas_size(&props.node) else {
            return rsx! {};
        };
        (1, h)
    } else {
        let Some(size) = canvas_size(&props.node) else {
            return rsx! {};
        };
        size
    };
    // As the web's OrbitalSceneCanvas: a filling canvas sits in a box that takes the slot's height (a canvas element
    // itself would fall back to its intrinsic ratio), drawn over the whole box; every touch on it is the scene's.
    let style = if props.fill_parent {
        format!("display:block;position:absolute;inset:0;width:100%;height:100%;touch-action:none;{}", props.inline_style)
    } else if props.fill_width {
        format!("display:block;width:100%;height:{h}px;touch-action:none;{}", props.inline_style)
    } else {
        format!("touch-action:none;{}", props.inline_style)
    };
    let sink = props.sink;
    // A repaint queues events of its own (measured text); deliver them on the next UI frame, not on the next pointer move.
    use_future({
        let host = host.clone();
        move || {
            let host = host.clone();
            async move {
                if sink.is_none() {
                    return;
                }
                loop {
                    crate::timer::Delay::new(std::time::Duration::from_millis(16)).await;
                    let emits = host.lock().expect("scene host poisoned").take_scene_events();
                    deliver(sink, emits);
                }
            }
        }
    });
    let class_name = props.class_name.clone();
    let canvas = rsx! {
        canvas {
            class: "{class_name}",
            style: style,
            aria_label: props.a11y.label,
            aria_labelledby: props.a11y.labelledby,
            aria_describedby: props.a11y.describedby,
            aria_live: props.a11y.live,
            aria_current: props.a11y.current,
            aria_pressed: props.a11y.pressed,
            aria_expanded: props.a11y.expanded,
            aria_selected: props.a11y.selected,
            aria_hidden: props.a11y.hidden,
            aria_busy: props.a11y.busy,
            role: props.a11y.role,
            lang: props.a11y.lang,
            dir: props.a11y.dir,
            tabindex: props.a11y.tabindex,
            "src": "{id}",
            width: w,
            height: h,
            onmousedown: {
                let host = host.clone();
                let held = held.clone();
                move |evt: MouseEvent| {
                    if held.load(Ordering::Relaxed) == 0 {
                        pointer(&host, sink, "touch-down", &evt)
                    }
                }
            },
            onmousemove: {
                let host = host.clone();
                let held = held.clone();
                move |evt: MouseEvent| {
                    if held.load(Ordering::Relaxed) > 0 {
                        return;
                    }
                    let kind = if evt.held_buttons().is_empty() {
                        "hover"
                    } else {
                        "touch-move"
                    };
                    pointer(&host, sink, kind, &evt);
                }
            },
            onmouseleave: {
                let host = host.clone();
                let held = held.clone();
                move |evt: MouseEvent| {
                    if held.load(Ordering::Relaxed) == 0 {
                        pointer(&host, sink, "leave", &evt)
                    }
                }
            },
            onmouseup: {
                let host = host.clone();
                let held = held.clone();
                move |evt: MouseEvent| {
                    if held.load(Ordering::Relaxed) == 0 {
                        pointer(&host, sink, "touch-up", &evt)
                    }
                }
            },
            ontouchstart: {
                let host = host.clone();
                let held = held.clone();
                move |evt: TouchEvent| touch(&host, sink, &held, "touch-down", &evt)
            },
            ontouchmove: {
                let host = host.clone();
                let held = held.clone();
                move |evt: TouchEvent| touch(&host, sink, &held, "touch-move", &evt)
            },
            ontouchend: {
                let host = host.clone();
                let held = held.clone();
                move |evt: TouchEvent| touch(&host, sink, &held, "touch-up", &evt)
            },
            ontouchcancel: {
                let host = host.clone();
                let held = held.clone();
                move |evt: TouchEvent| touch(&host, sink, &held, "touch-cancel", &evt)
            },
            onwheel: move |evt: WheelEvent| {
                let at = evt.element_coordinates();
                let (dx, dy) = match evt.delta() {
                    dioxus::html::geometry::WheelDelta::Pixels(v) => (v.x, v.y),
                    dioxus::html::geometry::WheelDelta::Lines(v) => (v.x * 16.0, v.y * 16.0),
                    dioxus::html::geometry::WheelDelta::Pages(_) => {
                        unreachable!("blitz delivers wheel deltas in pixels or lines")
                    }
                };
                let owned = {
                    let mut host = host.lock().expect("scene host poisoned");
                    host.wheel(at.x as f32, at.y as f32, dx as f32, dy as f32)
                        .then(|| host.take_scene_events())
                };
                if let Some(emits) = owned {
                    evt.prevent_default();
                    deliver(sink, emits);
                    return;
                }
                let delta_y_pixels = match evt.delta() {
                    dioxus::html::geometry::WheelDelta::Pixels(v) => v.y,
                    dioxus::html::geometry::WheelDelta::Lines(v) => v.y * 20.0,
                    dioxus::html::geometry::WheelDelta::Pages(_) => {
                        unreachable!("blitz delivers wheel deltas in pixels or lines")
                    }
                };
                let factor = 1.1f32.powf(-(delta_y_pixels as f32) / 100.0);
                let zoomed = host
                    .lock()
                    .expect("scene host poisoned")
                    .zoom(factor, at.x as f32, at.y as f32);
                if zoomed {
                    evt.prevent_default();
                }
            },
        }
    };
    if props.fill_parent {
        let box_style = format!(
            "position:relative;flex:1 1 0;width:100%;height:100%;min-height:{}px",
            props.min_height
        );
        rsx! { div { style: box_style, {canvas} } }
    } else {
        canvas
    }
}

/// One pointer event in element coordinates into the scene host (a 2D tap or drag, a 3D orbit or pick),
/// then the events it queued to the sink.
fn pointer(
    host: &Mutex<SceneHost>,
    sink: Option<crate::renderer::EventSink>,
    kind: &str,
    evt: &MouseEvent,
) {
    let at = evt.element_coordinates();
    let button = match evt.trigger_button() {
        Some(dioxus::html::input_data::MouseButton::Auxiliary) => 1,
        Some(dioxus::html::input_data::MouseButton::Secondary) => 2,
        _ => 0,
    };
    let emits = {
        let mut host = host.lock().expect("scene host poisoned");
        host.input(kind, at.x as f32, at.y as f32, button, None);
        host.take_scene_events()
    };
    deliver(sink, emits);
}

/// One touch event of the canvas: the finger that changed goes to the scene host as its own pointer, and the
/// count of fingers still down is kept so the mouse events a platform emulates from the same finger are
/// not read as a second pointer.
fn touch(
    host: &Mutex<SceneHost>,
    sink: Option<crate::renderer::EventSink>,
    held: &AtomicUsize,
    kind: &str,
    evt: &TouchEvent,
) {
    let data = evt.data();
    let Some(native) = data.downcast::<dioxus_native::NativeTouchData>() else {
        return;
    };
    let (id, at) = native.changed_in_element();
    held.store(native.touches_in_element().len(), Ordering::Relaxed);
    let emits = touch_points(host, kind, &[TouchPoint { id: pointer_id(id), x: at.x as f32, y: at.y as f32 }]);
    deliver(sink, emits);
}

/// The scene host's pointer id for a platform touch id. The mouse is pointer 0, so a finger never shares it.
pub(crate) fn pointer_id(touch_id: u64) -> i64 {
    touch_id as i64 + 1
}

/// One finger of a touch event: the host's id for it and its element coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TouchPoint {
    pub(crate) id: i64,
    pub(crate) x: f32,
    pub(crate) y: f32,
}

/// Every finger of one touch event into the scene host as its own pointer; the events they queued, in order.
pub(crate) fn touch_points(host: &Mutex<SceneHost>, kind: &str, points: &[TouchPoint]) -> Vec<scene::Emit> {
    let mut host = host.lock().expect("scene host poisoned");
    for point in points {
        host.input_pointer(kind, point.x, point.y, 0, None, point.id);
    }
    host.take_scene_events()
}

/// Send the scene events a host queued to the sink, in queue order.
fn deliver(sink: Option<crate::renderer::EventSink>, emits: Vec<scene::Emit>) {
    let Some(sink) = sink else { return };
    for emit in emits {
        match serde_json::from_value::<Value>(emit.delivered()) {
            Ok(payload) => sink.call((emit.event, payload)),
            Err(err) => tracing::warn!(target: NS, error = %err, "scene payload refused"),
        }
    }
}

/// Every scene pattern (`scene::scene_patterns`: learning canvases, diagrams, `svg-*` atoms and the
/// game `canvas`) — the hand-authored override, registered last so it wins by name over any
/// generated stub.
pub struct SceneCanvasRenderer {
    name: &'static str,
}

pub(crate) fn atlas_image(props: &Value, view: &DioxusView<'_>) -> Element {
    let node = scene_node("atlas-image", props, view.canvas_theme.as_ref());
    let (class_name, a11y) = scene_presentation(props);
    let fill_parent = props
        .as_object()
        .and_then(|fields| fields.get("fill"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inline_style = style_with_props("", props);
    rsx! { SceneCanvas { node, sink: None, fill_parent, a11y, class_name, inline_style } }
}

pub(crate) fn atlas_panel(props: &Value, view: &DioxusView<'_>, fill_parent: bool) -> Element {
    let mut node = (*scene_node("atlas-panel", props, view.canvas_theme.as_ref())).clone();
    if let Some(fields) = node.as_object_mut() {
        fields.remove("children");
    }
    let node = Arc::new(node);
    let (class_name, mut a11y) = scene_presentation(props);
    if fill_parent {
        a11y.hidden = Some("true".to_string());
    }
    let inline_style = if fill_parent {
        String::new()
    } else {
        style_with_props("", props)
    };
    rsx! { SceneCanvas { node, sink: None, fill_parent, a11y, class_name, inline_style } }
}

pub(crate) fn geometric_pattern_scene(props: &Value, view: &DioxusView<'_>) -> Element {
    let node = scene_node("geometric-pattern-scene", props, view.canvas_theme.as_ref());
    let mut a11y = SceneA11y::default();
    a11y.hidden = Some("true".to_string());
    rsx! { SceneCanvas {
        node,
        sink: None,
        fill_parent: true,
        a11y,
        class_name: "almadar-game-canvas",
        inline_style: String::new(),
    } }
}

impl DioxusPatternRenderer for SceneCanvasRenderer {
    fn pattern_name(&self) -> &str {
        self.name
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let node = scene_node(self.name, props, view.canvas_theme.as_ref());
        if learning::CARD_PATTERNS.contains(&self.name) {
            return learning_card(self.name, props, node, view, registry);
        }
        if self.name == "graph-view" {
            return graph_view_frame(props, node, view);
        }
        let (class_name, a11y) = scene_presentation(props);
        let inline_style = style_with_props("", props);
        let fill_parent = fills_container(&node);
        let fill_width = fills_width(&node);
        let min_height = min_height(&node);
        rsx! { SceneCanvas { node, sink: view.sink, fill_parent, fill_width, min_height, a11y, class_name, inline_style } }
    }
}

/// The web GraphView's root around the scene: a `relative overflow-hidden` box (a content surface
/// unless one is already painted around it), `width` px or the full width, `height` px (400), with
/// the scene filling it.
fn graph_view_frame(props: &Value, node: Arc<serde_json::Value>, view: &DioxusView<'_>) -> Element {
    use orbital_core::client::helpers::{get_number, get_string_or};
    let paints_surface = get_string_or(props, "surface", "auto") == "auto"
        && try_consume_context::<crate::components::core::atoms::card::OnSurface>().is_none();
    let surface = if paints_surface {
        "surface-content"
    } else {
        ""
    };
    let root_class = merge_class_name(&format!("relative overflow-hidden {surface}"), props);
    let width = match get_number(props, "width") {
        Some(width) => format!("{width}px"),
        None => "100%".to_string(),
    };
    let height = get_number(props, "height").unwrap_or(400.0);
    let root_style = format!("width:{width};height:{height}px");
    let (_, a11y) = scene_presentation(props);
    let sink = view.sink;
    rsx! {
        div { class: "{root_class}", style: "{root_style}",
            SceneCanvas {
                node,
                sink,
                fill_parent: true,
                a11y,
                class_name: "almadar-game-canvas",
                inline_style: String::new(),
            }
        }
    }
}

/// The card the web wraps around a learning molecule's canvas: the optional `h4` title over the
/// canvas, which scales to the card width at the scene's aspect ratio.
fn learning_card(
    pattern: &str,
    props: &Value,
    node: Arc<serde_json::Value>,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    let (w, h) = match scene::scene_size(&node) {
        Ok(size) => size,
        Err(err) => {
            tracing::warn!(target: NS, pattern, error = %err, "scene refused");
            return rsx! {};
        }
    };
    let card_class = crate::components::core::atoms::card::card_class(props);
    let title = props
        .as_object()
        .and_then(|fields| fields.get("title"))
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
        .map(|title| {
            let spec =
                serde_json::json!({ "type": "typography", "variant": "h4", "content": title });
            let spec = serde_json::from_value::<Value>(spec).expect("title spec");
            view.render_child(registry, &spec, None)
        });
    let inline_style = format!(
        "width:100%;max-width:min(100%,calc(70vh * {w} / {h}));height:auto;aspect-ratio:{w} / {h};"
    );
    let a11y = SceneA11y {
        role: Some("img".to_string()),
        ..SceneA11y::default()
    };
    let sink = view.sink;
    rsx! {
        div { class: "{card_class}",
            div { class: "flex flex-col gap-2",
                {title}
                SceneCanvas {
                    node,
                    sink,
                    fill_parent: false,
                    a11y,
                    class_name: "almadar-game-canvas mx-auto block touch-none rounded-container",
                    inline_style,
                }
            }
        }
    }
}

pub(crate) fn fill_parent_atlas_panel(asset: &Value, mode: &str, view: &DioxusView<'_>) -> Element {
    let props = serde_json::json!({ "asset": asset, "mode": mode });
    let props = serde_json::from_value::<Value>(props).expect("atlas panel props");
    atlas_panel(&props, view, true)
}

/// Renderer for the `game-hud` pattern — stats/items/elements resolved against
/// the entity cache.
pub struct GameHudRenderer;

impl DioxusPatternRenderer for GameHudRenderer {
    fn pattern_name(&self) -> &str {
        "game-hud"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let stats = get_hud_stats(props);
        rsx! {
            div { class: "almadar-game-hud",
                if stats.is_empty() {
                    span { class: "almadar-game-hud-empty", "No stats configured" }
                } else {
                    for stat in stats.iter() {
                        {
                            let value = view
                                .entities
                                .get(&stat.source)
                                .and_then(|items| items.first())
                                .and_then(|item| get_entity_field(item, &stat.field))
                                .map(|v| format_value(&v))
                                .unwrap_or_else(|| "-".to_string());
                            rsx! {
                                span { class: "almadar-game-hud-stat",
                                    "{humanize_field_name(&stat.field)}: {value}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Register the hand-authored game renderers. Called by the shell AFTER the
/// generated register sets so these win by name.
pub fn register_overrides(registry: &mut DioxusPatternRegistry) {
    for name in scene::scene_patterns() {
        registry.register(Box::new(SceneCanvasRenderer { name }));
    }
    registry.register(Box::new(GameHudRenderer));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_canvas_box_with_no_area_is_not_drawn_and_a_laid_out_box_is() {
        assert!(!box_has_area(1035, 0), "a box laid out to no height yet (the first frame on a phone) is not drawn");
        assert!(!box_has_area(0, 2164));
        assert!(box_has_area(1001, 2164), "control: a laid-out box is drawn");
        assert!(box_has_area(1, 1), "edge: one pixel is an area");
    }

    #[test]
    fn the_same_kernel_value_is_converted_to_a_scene_node_once() {
        let props: Value = serde_json::from_value(json!({ "width": 640, "height": 360, "drawables": [] })).unwrap();
        let first = scene_node("canvas", &props, None);
        let again = scene_node("canvas", &props.clone(), None);
        assert!(Arc::ptr_eq(&first, &again), "an unchanged scene (the same allocation) is not converted again");
        let changed: Value = serde_json::from_value(json!({ "width": 641, "height": 360, "drawables": [] })).unwrap();
        let next = scene_node("canvas", &changed, None);
        assert!(!Arc::ptr_eq(&first, &next), "control: a new scene is converted");
        assert_eq!(next["width"], json!(641));
        let theme = json!({ "palette": { "ink": "#000" } });
        assert!(!Arc::ptr_eq(&next, &scene_node("canvas", &changed, Some(&theme))), "control: a new theme is converted");
    }

    #[test]
    fn a_ready_frame_counts_only_once_its_assets_have_landed() {
        assert!(!drawn_ready(true, 2, false), "a ready frame drawn while fonts download is not the laid-out scene");
        assert!(drawn_ready(true, 0, false), "control: a ready frame with everything landed is");
        assert!(!drawn_ready(false, 0, false), "control: a frame the program did not mark ready is not");
    }

    #[test]
    fn a_ready_frame_that_queued_sizes_the_program_has_not_seen_is_not_ready() {
        assert!(!drawn_ready(true, 0, true), "the frame measured new sizes: the program lays out again before the page is final");
        assert!(drawn_ready(true, 0, false), "control: nothing left to tell the program");
    }

    fn touch_scene() -> Mutex<SceneHost> {
        let mut host = SceneHost::new();
        host.set_scene(json!({
            "type": "canvas", "mode": "2d", "projection": "free", "camera": {"zoom": 1},
            "pointerMoves": "all", "pointerMoveEvent": "MOVE", "pointerDownEvent": "DOWN", "pointerUpEvent": "UP",
            "drawables": [{"type": "draw-shape", "id": "b", "shape": "rect", "anchor": "top-left",
                "position": {"x": 0, "y": 0}, "width": 100, "height": 100, "fill": "#000000", "hit": true}]
        }))
        .unwrap();
        Mutex::new(host)
    }

    fn touch_events(host: &Mutex<SceneHost>, kind: &str, points: &[TouchPoint]) -> Vec<scene::Emit> {
        touch_points(host, kind, points)
    }

    #[test]
    fn two_touch_points_reach_the_scene_host_as_two_pointers() {
        let host = touch_scene();
        touch_events(&host, "touch-down", &[TouchPoint { id: 1, x: 10.0, y: 10.0 }]);
        touch_events(&host, "touch-down", &[TouchPoint { id: 2, x: 60.0, y: 50.0 }]);
        let moved = touch_events(
            &host,
            "touch-move",
            &[TouchPoint { id: 1, x: 10.0, y: 10.0 }, TouchPoint { id: 2, x: 70.0, y: 50.0 }],
        );
        let carried = moved.iter().filter_map(|e| e.payload.get("touches")).last().expect("touches payload");
        let ids: Vec<_> = carried.as_array().unwrap().iter().map(|t| t["pointerId"].clone()).collect();
        assert_eq!(ids, [json!(1), json!(2)], "{carried}");
    }

    #[test]
    fn a_finger_never_takes_the_mouse_pointer_id() {
        assert_ne!(pointer_id(0), 0);
        assert_eq!(pointer_id(7), 8);
    }

    #[test]
    fn control_one_touch_point_is_one_pointer_with_no_touches_payload() {
        let host = touch_scene();
        let down = touch_events(&host, "touch-down", &[TouchPoint { id: 1, x: 10.0, y: 10.0 }]);
        let moved = touch_events(&host, "touch-move", &[TouchPoint { id: 1, x: 20.0, y: 10.0 }]);
        assert!(!down.is_empty() && !moved.is_empty());
        assert!(down.iter().chain(&moved).all(|e| e.payload.get("touches").is_none()));
    }

    #[test]
    fn a_canvas_takes_its_scenes_own_size_and_carries_the_kernel_theme() {
        let props: Value = serde_json::from_value(json!({ "width": 640, "height": 360 })).unwrap();
        let theme = json!({ "ink": "#111111" });
        let node = scene_node("canvas", &props, Some(&theme));
        assert_eq!(node["canvasTheme"], theme);
        assert_eq!(canvas_size(&node), Some((640, 360)));
    }

    #[test]
    fn an_orbital_canvas_fills_its_parent_only_when_it_asks_to() {
        let fill: Value = serde_json::from_value(
            json!({ "painter": "orbital", "fillContainer": true, "width": 640, "height": 360 }),
        )
        .unwrap();
        assert!(fills_container(&scene_node("canvas", &fill, None)));
        let own: Value =
            serde_json::from_value(json!({ "painter": "orbital", "width": 640, "height": 360 }))
                .unwrap();
        assert!(!fills_container(&scene_node("canvas", &own, None)));
        let other: Value =
            serde_json::from_value(json!({ "fillContainer": true, "width": 640, "height": 360 }))
                .unwrap();
        assert!(!fills_container(&scene_node("canvas", &other, None)));
    }

    #[test]
    fn an_orbital_canvas_takes_the_parent_width_and_its_own_height_when_it_asks_for_width() {
        let width: Value = serde_json::from_value(
            json!({ "painter": "orbital", "fillContainer": "width", "width": 640, "height": 1800 }),
        )
        .unwrap();
        let node = scene_node("canvas", &width, None);
        assert!(fills_width(&node));
        assert!(!fills_container(&node), "width is not the whole box");
        let fill: Value = serde_json::from_value(
            json!({ "painter": "orbital", "fillContainer": true, "width": 640, "height": 360 }),
        )
        .unwrap();
        assert!(!fills_width(&scene_node("canvas", &fill, None)));
        let other: Value =
            serde_json::from_value(json!({ "fillContainer": "width", "width": 640, "height": 360 }))
                .unwrap();
        assert!(!fills_width(&scene_node("canvas", &other, None)), "only the orbital painter");
    }

    #[test]
    fn a_filled_orbital_canvas_keeps_its_declared_min_height() {
        let tall: Value = serde_json::from_value(
            json!({ "painter": "orbital", "fillContainer": true, "minHeight": 600, "width": 640, "height": 360 }),
        )
        .unwrap();
        assert_eq!(min_height(&scene_node("canvas", &tall, None)), 600.0);
        let plain: Value = serde_json::from_value(
            json!({ "painter": "orbital", "fillContainer": true, "width": 640, "height": 360 }),
        )
        .unwrap();
        assert_eq!(min_height(&scene_node("canvas", &plain, None)), 0.0, "control: none declared");
    }

    #[test]
    fn a_three_d_scene_takes_its_authored_room_now_the_painter_draws_it() {
        let props: Value = serde_json::from_value(json!({ "mode": "3d" })).unwrap();
        assert_eq!(canvas_size(&scene_node("canvas", &props, None)), Some((800, 600)));
    }

    #[test]
    fn pattern_tile_projects_authored_entry_and_accessibility_without_a_window() {
        let spec: Value = serde_json::from_value(json!({
            "type":"pattern-tile", "variant":"star8", "size":80,
            "color":"#101010", "strokeWidth":2,
            "className":"tile-accent", "enter":"fade", "enterDelay":2,
            "aria-label":"Eight-point star", "aria-labelledby":"tile-title",
            "aria-describedby":"tile-description", "aria-live":"polite",
            "aria-current":"step", "aria-pressed":"mixed", "aria-expanded":true,
            "aria-selected":false, "aria-hidden":false, "aria-busy":true,
            "role":"img", "lang":"ar", "dir":"rtl", "tabIndex":0
        }))
        .unwrap();
        let entry = crate::registry::entry_props(&spec).unwrap();
        let (class_name, a11y) = scene_presentation(&entry);
        assert_eq!(
            class_name,
            "almadar-game-canvas tile-accent almadar-enter-fade almadar-enter-delay-2"
        );
        assert_eq!(
            a11y,
            SceneA11y {
                label: Some("Eight-point star".into()),
                labelledby: Some("tile-title".into()),
                describedby: Some("tile-description".into()),
                live: Some("polite".into()),
                current: Some("step".into()),
                pressed: Some("mixed".into()),
                expanded: Some("true".into()),
                selected: Some("false".into()),
                hidden: Some("false".into()),
                busy: Some("true".into()),
                role: Some("img".into()),
                lang: Some("ar".into()),
                dir: Some("rtl".into()),
                tabindex: Some("0".into()),
            }
        );
    }
}
