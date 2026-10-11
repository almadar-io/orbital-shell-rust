//! This renderer as the target of the Layer 2 render probe
//! (`orbital_core::client::render_probe`).

use crate::registry::{default_registry, DioxusPatternRegistry};
use orbital_core::client::render_probe::RenderTarget;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use orbital_core::{client::render_probe::GameCanvasOutcome, Value};

/// The Dioxus registry as a probe target; game canvases paint through the
/// headless game renderer on native builds.
pub struct DioxusTarget {
    registry: DioxusPatternRegistry,
}

impl DioxusTarget {
    pub fn new() -> Self {
        Self { registry: default_registry() }
    }
}

impl Default for DioxusTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderTarget for DioxusTarget {
    fn supports(&self, pattern: &str) -> bool {
        self.registry.supports(pattern)
    }

    fn is_stub(&self, pattern: &str) -> bool {
        self.registry.is_stub(pattern)
    }

    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    fn probe_game_canvas(&self, pattern: &str, props: &Value) -> GameCanvasOutcome {
        paint_game_canvas(pattern, props)
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use orbital_canvas::{mappers, Canvas, RendererError};

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
/// Build a `GameRenderPacket` from a game-canvas pattern's props, render one frame through
/// the headless wgpu path, and assert the packet has draw commands and the RGBA buffer is
/// non-empty + correctly sized. A missing GPU adapter is a `Skip`, never a `Fail`.
fn paint_game_canvas(pattern: &str, props: &Value) -> GameCanvasOutcome {
    let serde_props = serde_json::to_value(props).unwrap_or(serde_json::Value::Null);
    let packet = match pattern {
        scene if mappers::scene::is_scene_pattern(scene) => {
            let mut node = serde_props;
            if let Some(fields) = node.as_object_mut() {
                fields.insert(
                    "type".to_string(),
                    serde_json::Value::String(scene.to_string()),
                );
            }
            match mappers::scene::map_scene(&node, None) {
                Ok(mapped) => mapped.packet,
                Err(e) => {
                    return GameCanvasOutcome::Fail(format!("`{pattern}`: scene refused: {e}"))
                }
            }
        }
        "mini-map" => mappers::mini_map::from_props(&serde_props),
        other => return GameCanvasOutcome::Skip(format!("no mapper for `{other}`")),
    };
    if packet.draw_commands.is_empty() {
        return GameCanvasOutcome::Fail(format!(
            "`{pattern}`: mapper produced 0 draw commands (blank packet)"
        ));
    }
    let mut renderer = match Canvas::new_headless() {
        Ok(r) => r,
        Err(e) => {
            return GameCanvasOutcome::Skip(format!(
                "`{pattern}`: renderer construction failed (env gap): {e}"
            ))
        }
    };
    match renderer.render_packet_to_rgba(&packet) {
        Ok(rgba) => {
            let expected = (packet.width.max(1.0) as usize) * (packet.height.max(1.0) as usize) * 4;
            if rgba.is_empty() {
                GameCanvasOutcome::Fail(format!(
                    "`{pattern}`: empty RGBA buffer (expected {expected} bytes)"
                ))
            } else if rgba.len() != expected {
                GameCanvasOutcome::Fail(format!(
                    "`{pattern}`: RGBA length {} ≠ expected {expected} ({}×{}×4)",
                    rgba.len(),
                    packet.width as u32,
                    packet.height as u32
                ))
            } else {
                GameCanvasOutcome::Pass
            }
        }
        Err(RendererError::NoAdapter) => GameCanvasOutcome::Skip(format!(
            "`{pattern}`: no GPU adapter (headless CI) — skipped"
        )),
        Err(RendererError::DeviceRequestFailed(msg)) => GameCanvasOutcome::Skip(format!(
            "`{pattern}`: GPU device init failed (env gap): {msg}"
        )),
        Err(e) => GameCanvasOutcome::Fail(format!("`{pattern}`: render error: {e}")),
    }
}
