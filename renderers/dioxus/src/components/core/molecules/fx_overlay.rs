//! Native Dioxus renderer for the `fx-overlay` pattern: the web's screen-space CSS effect layer.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, merge_class_name, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `fx-overlay` pattern.
pub struct FxOverlayRenderer;

impl FxOverlayRenderer {
    const NAME: &str = "fx-overlay";
}

const DEFAULT_TICK_MS: f64 = 500.0;
const BOX: &str = "bg-transparent rounded-none shadow-none";
const CONFETTI_COLORS: [&str; 6] = [
    "var(--color-primary)",
    "var(--color-success)",
    "var(--color-warning)",
    "var(--color-error)",
    "gold",
    "dodgerblue",
];

const FX_OVERLAY_KEYFRAMES: &str = "
@keyframes confetti-burst {
  0% {
    opacity: 1;
    transform: translate(0, 0) rotate(0deg) scale(1);
  }
  70% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: translate(var(--confetti-tx), var(--confetti-ty)) rotate(var(--confetti-rotate)) scale(0.5);
  }
}

@keyframes fx-overlay-flash {
  0% { opacity: 0.75; }
  100% { opacity: 0; }
}
@keyframes fx-overlay-glow {
  0% { opacity: 0.9; }
  60% { opacity: 0.6; }
  100% { opacity: 0; }
}
@keyframes fx-overlay-sparkle {
  0% { opacity: 0; transform: scale(0); }
  30% { opacity: 1; transform: scale(1); }
  100% { opacity: 0; transform: scale(0.3); }
}
@keyframes fx-overlay-float {
  0% { opacity: 0; transform: translate(-50%, 8px); }
  15% { opacity: 1; }
  100% { opacity: 0; transform: translate(-50%, -40px); }
}
@keyframes fx-overlay-shake {
  0% { transform: translateX(0); }
  15% { transform: translateX(-6px); }
  30% { transform: translateX(5px); }
  45% { transform: translateX(-4px); }
  60% { transform: translateX(3px); }
  75% { transform: translateX(-2px); }
  100% { transform: translateX(0); }
}
";

struct FxItem {
    id: String,
    effect: Option<String>,
    message: Option<String>,
    color: Option<String>,
    size: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    ttl: f64,
    max_ttl: Option<f64>,
    particle_count: Option<usize>,
}

impl FxItem {
    fn from_value(item: &Value) -> Option<FxItem> {
        if get_string_or(item, "space", "screen") == "world" {
            return None;
        }
        let id = get_string(item, "id")?;
        let Some(ttl) = get_number(item, "ttl") else {
            tracing::error!(id = %id, "fx-overlay item has no numeric ttl");
            return None;
        };
        Some(FxItem {
            id,
            effect: get_string(item, "effect"),
            message: get_string(item, "message"),
            color: get_string(item, "color"),
            size: get_number(item, "size"),
            x: get_number(item, "x"),
            y: get_number(item, "y"),
            z: get_number(item, "z"),
            ttl,
            max_ttl: get_number(item, "maxTtl"),
            particle_count: get_number(item, "particleCount").map(|count| count.max(0.0) as usize),
        })
    }

    fn life_ms(&self, tick_ms: f64) -> f64 {
        self.max_ttl.unwrap_or(self.ttl).max(self.ttl).max(1.0) * tick_ms
    }

    fn left(&self) -> f64 {
        self.x.unwrap_or(0.5) * 100.0
    }

    fn top(&self) -> f64 {
        self.z.or(self.y).unwrap_or(0.4) * 100.0
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
fn hash01(seed: &str, salt: u32) -> Option<f64> {
    Some(orbital_canvas::fx_projection::fx_hash01(seed, salt))
}

#[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
fn hash01(_seed: &str, _salt: u32) -> Option<f64> {
    None
}

fn confetti_burst(item: &FxItem, life_ms: f64) -> Element {
    let count = item.particle_count.unwrap_or(30);
    let scale = item.size.unwrap_or(1.0);
    let anchor = format!("left:{}%;top:{}%", item.left(), item.top());
    let class = cn(&[BOX, "absolute rounded-sm"]);
    let mut particles = Vec::with_capacity(count);
    for i in 0..count {
        let salt = |n: usize| (i * 7 + n) as u32;
        let (Some(h1), Some(h2), Some(h3), Some(h4), Some(h5), Some(h6), Some(h7)) = (
            hash01(&item.id, salt(1)),
            hash01(&item.id, salt(2)),
            hash01(&item.id, salt(3)),
            hash01(&item.id, salt(4)),
            hash01(&item.id, salt(5)),
            hash01(&item.id, salt(6)),
            hash01(&item.id, salt(7)),
        ) else {
            tracing::error!(
                "fx-overlay confetti needs the fx hash, which this build does not carry"
            );
            break;
        };
        let default_color = CONFETTI_COLORS[(h1 * CONFETTI_COLORS.len() as f64).floor() as usize];
        let left = 30.0 + h2 * 40.0;
        let delay = h3 * 300.0;
        let angle = h4 * 360.0;
        let distance = 40.0 + h5 * 80.0;
        let rotation = h6 * 720.0 - 360.0;
        let size = 4.0 + h7 * 6.0;
        let rad = angle * std::f64::consts::PI / 180.0;
        let tx = rad.cos() * distance * scale;
        let ty = rad.sin() * distance * scale - 20.0;
        let color = item
            .color
            .clone()
            .unwrap_or_else(|| default_color.to_string());
        let duration = (life_ms - delay).max(200.0);
        let style = format!(
            "left:{}px;top:-10px;width:{size}px;height:{size}px;background-color:{color};animation:confetti-burst {duration}ms ease-out {delay}ms forwards;opacity:0;--confetti-tx:{tx}px;--confetti-ty:{ty}px;--confetti-rotate:{rotation}deg",
            (left - 50.0) * 2.0
        );
        particles.push(style);
    }
    rsx! {
        div { class: "{BOX} absolute", style: "{anchor}",
            for (index, style) in particles.iter().enumerate() {
                div { key: "{index}", class: "{class}", style: "{style}" }
            }
        }
    }
}

fn sparkle_burst(item: &FxItem, life_ms: f64) -> Element {
    let count = item.particle_count.unwrap_or(8);
    let scale = item.size.unwrap_or(1.0);
    let anchor = format!("left:{}%;top:{}%", item.left(), item.top());
    let class = cn(&[BOX, "absolute rounded-full"]);
    let color = item.color.clone().unwrap_or_else(|| "gold".to_string());
    let mut dots = Vec::with_capacity(count);
    for i in 0..count {
        let salt = |n: usize| (i * 4 + n) as u32;
        let (Some(h0), Some(h1), Some(h2), Some(h3)) = (
            hash01(&item.id, salt(0)),
            hash01(&item.id, salt(1)),
            hash01(&item.id, salt(2)),
            hash01(&item.id, salt(3)),
        ) else {
            tracing::error!(
                "fx-overlay sparkle needs the fx hash, which this build does not carry"
            );
            break;
        };
        let dx = (h0 - 0.5) * 160.0 * scale;
        let dy = (h1 - 0.5) * 120.0 * scale;
        let dot = (4.0 + h2 * 5.0) * scale;
        let delay = h3 * life_ms * 0.4;
        let duration = (life_ms - delay).max(200.0);
        dots.push(format!(
            "left:{dx}px;top:{dy}px;width:{dot}px;height:{dot}px;background-color:{color};opacity:0;animation:fx-overlay-sparkle {duration}ms ease-in-out {delay}ms forwards"
        ));
    }
    rsx! {
        div { class: "{BOX} absolute", style: "{anchor}",
            for (index, style) in dots.iter().enumerate() {
                div { key: "{index}", class: "{class}", style: "{style}" }
            }
        }
    }
}

fn overlay_node(item: &FxItem, tick_ms: f64) -> Element {
    let life_ms = item.life_ms(tick_ms);
    match item.effect.as_deref().unwrap_or("sparkle") {
        "confetti" => confetti_burst(item, life_ms),
        "flash" => {
            let style = format!(
                "background-color:{};opacity:0;animation:fx-overlay-flash {life_ms}ms ease-out forwards",
                item.color.as_deref().unwrap_or("#ffffff")
            );
            rsx! { div { class: "{BOX} absolute inset-0", style: "{style}" } }
        }
        "streak-glow" => {
            let style = format!(
                "box-shadow:inset 0 0 {}px {};opacity:0;animation:fx-overlay-glow {life_ms}ms ease-in-out forwards",
                60.0 * item.size.unwrap_or(1.0),
                item.color.as_deref().unwrap_or("var(--color-warning)")
            );
            rsx! { div { class: "{BOX} absolute inset-0", style: "{style}" } }
        }
        "float-text" => {
            let Some(message) = item
                .message
                .as_deref()
                .filter(|message| !message.is_empty())
            else {
                return rsx! {};
            };
            let style = format!(
                "left:{}%;top:{}%;color:{};font-weight:700;font-size:{}px;text-shadow:0 1px 2px rgba(0,0,0,0.4);opacity:0;animation:fx-overlay-float {life_ms}ms ease-out forwards;white-space:nowrap",
                item.left(),
                item.top(),
                item.color.as_deref().unwrap_or("var(--color-success)"),
                16.0 * item.size.unwrap_or(1.0)
            );
            rsx! { div { class: "{BOX} absolute", style: "{style}", "{message}" } }
        }
        "shake" => rsx! {},
        _ => sparkle_burst(item, life_ms),
    }
}

impl DioxusPatternRenderer for FxOverlayRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let tick_ms = get_number_or(props, "tickMs", DEFAULT_TICK_MS);
        let items: Vec<FxItem> = get_array(props, "items")
            .iter()
            .filter_map(FxItem::from_value)
            .collect();
        let shake = items
            .iter()
            .rev()
            .find(|item| item.effect.as_deref() == Some("shake"));
        let has_children = props
            .get("children")
            .is_some_and(|children| !matches!(children, Value::Null));
        let overlay_class = if has_children {
            cn(&[
                BOX,
                "absolute",
                "inset-0 pointer-events-none overflow-hidden z-50",
            ])
        } else {
            merge_class_name(
                &format!("{BOX} absolute inset-0 pointer-events-none overflow-hidden z-50"),
                props,
            )
        };
        let overlay_attrs = if has_children {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let overlay = rsx! {
            div { class: "{overlay_class}", "aria-hidden": "true", ..overlay_attrs,
                for item in items.iter() {
                    {rsx! { Fragment { key: "{item.id}", {overlay_node(item, tick_ms)} } }}
                }
                style { "{FX_OVERLAY_KEYFRAMES}" }
            }
        };
        if !has_children {
            return overlay;
        }
        let root_class = merge_class_name(&format!("{BOX} relative"), props);
        let shake_key = shake
            .map(|item| item.id.clone())
            .unwrap_or_else(|| "steady".to_string());
        let shake_style = shake
            .map(|item| {
                format!(
                    "animation:fx-overlay-shake {}ms ease-in-out",
                    item.life_ms(tick_ms).min(600.0)
                )
            })
            .unwrap_or_default();
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{root_class}", ..shared_root_attrs(props),
                div { key: "{shake_key}", class: "{BOX}", style: "{shake_style}",
                    for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                }
                {overlay}
            }
        }
    }
}
