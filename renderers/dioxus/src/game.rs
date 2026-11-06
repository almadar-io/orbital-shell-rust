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
use std::sync::{Arc, Mutex};
use web_time::Instant;

/// Log namespace — mirrors `@almadar/logger`'s `almadar:ui-native-game`.
const NS: &str = "almadar::ui_native_game";

/// The node a scene pattern's host draws: its props under its `type`, plus the kernel's theme.
fn scene_node(
    pattern: &str,
    props: &Value,
    canvas_theme: Option<&serde_json::Value>,
) -> serde_json::Value {
    scene::host_node(
        pattern,
        serde_json::to_value(props).unwrap_or_default(),
        canvas_theme,
    )
}

/// The canvas element's size in CSS px: the scene's own size. A scene the native painter refuses
/// (`mode: "3d"`) is logged and takes no room.
fn canvas_size(node: &serde_json::Value) -> Option<(usize, usize)> {
    match scene::scene_size(node) {
        Ok((w, h)) => Some((w.max(1.0) as usize, h.max(1.0) as usize)),
        Err(err) => {
            tracing::warn!(target: NS, error = %err, "scene refused");
            None
        }
    }
}

/// Paints one scene into the Blitz scene via the shared wgpu device. The scene host is shared
/// with the component, which feeds it taps; this side keeps its viewport, clock, assets and fonts
/// current and draws its frame. Owns the offscreen target texture (recreated on size change).
struct GamePaintSource {
    node: Arc<Mutex<serde_json::Value>>,
    host: Arc<Mutex<SceneHost>>,
    renderer: Option<SharedDeviceRenderer>,
    target: Option<(wgpu::Texture, TextureHandle, u32, u32)>,
    started: Instant,
    renders: u32,
}

impl GamePaintSource {
    fn new(node: Arc<Mutex<serde_json::Value>>, host: Arc<Mutex<SceneHost>>) -> Self {
        Self {
            node,
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
        let renderer = self.renderer.as_mut()?;
        // Blitz reports the canvas content box ALREADY in physical px (CSS px
        // × dpr — proven: `game paint render css=2000x1500` for an 800x600
        // logical canvas at 2.5 dpr). Render 1:1 at that size; vello stretches
        // the registered texture back over the box (`scene.rs`
        // `render_custom_source`). Do NOT multiply by `scale` again — that
        // double-scales the target (5000x3750 = 6.25× the needed pixels).
        let phys_w = width.max(1);
        let phys_h = height.max(1);

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

        let node = self.node.lock().expect("scene node poisoned").clone();
        let packet = {
            let mut host = self.host.lock().expect("scene host poisoned");
            let density = scale as f32;
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
            if host.scene() != Some(&node) || fonts_landed {
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
            host.frame_packet()?
        };
        let (texture, handle, _, _) = self.target.as_ref()?;
        let ok = renderer.render_packet_to_texture(&packet, texture);
        self.renders += 1;
        if self.renders <= 3 || self.renders % 600 == 0 {
            tracing::info!(
                target: NS,
                n = self.renders,
                phys = %format_args!("{width}x{height}"),
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
    node: serde_json::Value,
    sink: Option<crate::renderer::EventSink>,
    fill_parent: bool,
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
    let node_cell = use_hook(|| Arc::new(Mutex::new(props.node.clone())));
    let host = use_hook(|| Arc::new(Mutex::new(SceneHost::new())));
    // Feed the node every vdom render; the paint source re-maps when it changes.
    *node_cell.lock().expect("scene node poisoned") = props.node.clone();
    let id = use_wgpu({
        let (node_cell, host) = (node_cell.clone(), host.clone());
        move || GamePaintSource::new(node_cell, host)
    });
    let (w, h) = if props.fill_parent {
        (1, 1)
    } else {
        let Some(size) = canvas_size(&props.node) else {
            return rsx! {};
        };
        size
    };
    let style = if props.fill_parent {
        format!(
            "display:block;width:100%;height:100%;{}",
            props.inline_style
        )
    } else {
        props.inline_style.clone()
    };
    let sink = props.sink;
    let class_name = props.class_name.clone();
    rsx! {
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
                move |evt: MouseEvent| pointer(&host, sink, "touch-down", &evt)
            },
            onmousemove: {
                let host = host.clone();
                move |evt: MouseEvent| {
                    if !evt.held_buttons().is_empty() {
                        pointer(&host, sink, "touch-move", &evt);
                    }
                }
            },
            onmouseup: {
                let host = host.clone();
                move |evt: MouseEvent| pointer(&host, sink, "touch-up", &evt)
            },
            onwheel: move |evt: WheelEvent| {
                let delta_y_pixels = match evt.delta() {
                    dioxus::html::geometry::WheelDelta::Pixels(v) => v.y,
                    dioxus::html::geometry::WheelDelta::Lines(v) => v.y * 20.0,
                    dioxus::html::geometry::WheelDelta::Pages(_) => {
                        unreachable!("blitz delivers wheel deltas in pixels or lines")
                    }
                };
                let at = evt.element_coordinates();
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
    let mut node = scene_node("atlas-panel", props, view.canvas_theme.as_ref());
    if let Some(fields) = node.as_object_mut() {
        fields.remove("children");
    }
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
        rsx! { SceneCanvas { node, sink: view.sink, fill_parent: false, a11y, class_name, inline_style } }
    }
}

/// The web GraphView's root around the scene: a `relative overflow-hidden` box (a content surface
/// unless one is already painted around it), `width` px or the full width, `height` px (400), with
/// the scene filling it.
fn graph_view_frame(props: &Value, node: serde_json::Value, view: &DioxusView<'_>) -> Element {
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
    node: serde_json::Value,
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
    fn a_canvas_takes_its_scenes_own_size_and_carries_the_kernel_theme() {
        let props: Value = serde_json::from_value(json!({ "width": 640, "height": 360 })).unwrap();
        let theme = json!({ "ink": "#111111" });
        let node = scene_node("canvas", &props, Some(&theme));
        assert_eq!(node["canvasTheme"], theme);
        assert_eq!(canvas_size(&node), Some((640, 360)));
    }

    #[test]
    fn control_a_refused_three_d_scene_takes_no_room() {
        let props: Value = serde_json::from_value(json!({ "mode": "3d" })).unwrap();
        assert_eq!(canvas_size(&scene_node("canvas", &props, None)), None);
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
