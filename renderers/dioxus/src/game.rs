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
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use anyrender_vello::wgpu;
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use dioxus::prelude::*;
use dioxus_native::use_wgpu;
use orbital_core::Value;
use orbital_canvas::mappers::scene;
use orbital_canvas::{SceneHost, SharedDeviceRenderer};
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
        self.renderer = Some(SharedDeviceRenderer::new(
            Arc::new(device_handle.device.clone()),
            Arc::new(device_handle.queue.clone()),
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
            let fonts_landed =
                orbital_canvas::system_fonts::supply_missing_from_system() > 0;
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
    let Some((w, h)) = canvas_size(&props.node) else {
        return rsx! {};
    };
    let sink = props.sink;
    rsx! {
        canvas {
            class: "almadar-game-canvas",
            "src": "{id}",
            width: w,
            height: h,
            onclick: move |evt: MouseEvent| {
                let at = evt.element_coordinates();
                let (x, y) = (at.x as f32, at.y as f32);
                let emits = {
                    let mut host = host.lock().expect("scene host poisoned");
                    host.input("touch-down", x, y, 0, None);
                    host.input("touch-up", x, y, 0, None);
                    host.take_scene_events()
                };
                let Some(sink) = sink else { return };
                for emit in emits {
                    match serde_json::from_value::<Value>(emit.delivered()) {
                        Ok(payload) => sink.call((emit.event, payload)),
                        Err(err) => tracing::warn!(target: NS, error = %err, "scene payload refused"),
                    }
                }
            },
        }
    }
}

/// Every scene pattern (`scene::scene_patterns`: learning canvases, diagrams, `svg-*` atoms and the
/// game `canvas`) — the hand-authored override, registered last so it wins by name over any
/// generated stub.
pub struct SceneCanvasRenderer {
    name: &'static str,
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
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let node = scene_node(self.name, props, view.canvas_theme.as_ref());
        rsx! { SceneCanvas { node, sink: view.sink } }
    }
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
}
