//! Native geometric pattern projection from the shared tile geometry.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `geometric-pattern` pattern.
pub struct GeometricPatternRenderer;

impl GeometricPatternRenderer {
    const NAME: &str = "geometric-pattern";
}

impl DioxusPatternRenderer for GeometricPatternRenderer {
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
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        {
            let class = merge_class_name("", props);
            let mode = get_string_or(props, "mode", "background");
            if mode == "frame" {
                let scale = get_number_or(props, "scale", 1.0);
                let strip_style = format!("height:{}px", 40.0 * scale);
                let children = child_patterns(props, view.trait_content);
                return rsx! {
                    div { class: "relative {class}",
                        div { class: "w-full pointer-events-none", style: "{strip_style}", {crate::game::geometric_pattern_scene(props, view)} }
                        for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                        div { class: "w-full pointer-events-none", style: "{strip_style}", {crate::game::geometric_pattern_scene(props, view)} }
                    }
                };
            }
            return rsx! {
                div { class: "absolute inset-0 w-full h-full pointer-events-none {class}",
                    {crate::game::geometric_pattern_scene(props, view)}
                }
            };
        }
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        {
            let __cn_merged = merge_class_name("", props);
            let mode = get_string_or(props, "mode", "background");
            let opacity = get_number_or(props, "opacity", 0.06);
            let scale = get_number_or(props, "scale", 1.0);
            let mut tile_props = orbital_core::ValueMap::new();
            tile_props.insert(
                "type".to_string().into(),
                Value::String(("pattern-tile".to_string()).into()),
            );
            tile_props.insert(
                "variant".to_string().into(),
                Value::String(get_string_or(props, "variant", "star8").into()),
            );
            tile_props.insert("size".to_string().into(), Value::Number(60.0 * scale));
            tile_props.insert(
                "color".to_string().into(),
                Value::String(get_string_or(props, "color", "var(--color-primary)").into()),
            );
            tile_props.insert(
                "strokeWidth".to_string().into(),
                Value::Number(get_number_or(props, "strokeWidth", 0.5)),
            );
            let tile = Value::Object(tile_props.into());
            let frame = mode == "frame";
            let mask = match mode.as_str() {
            "left" => "mask-image:linear-gradient(to right, black 0%, transparent 60%);",
            "right" => "mask-image:linear-gradient(to left, black 0%, transparent 60%);",
            "dual" => "mask-image:linear-gradient(to right, black 0%, transparent 35%, transparent 65%, black 100%);",
            "around" => "mask-image:radial-gradient(circle at center, transparent 0%, transparent 55%, black 85%, black 100%);",
            _ => "",
        };
            let layer_style = format!("opacity:{opacity};{mask}");
            let strip_style = format!("height:{}px;opacity:{opacity}", 40.0 * scale);
            let root_cls = if frame {
                "relative"
            } else {
                "absolute inset-0 w-full h-full pointer-events-none"
            };
            let children = child_patterns(props, view.trait_content);
            rsx! {
                div { class: "{root_cls} {__cn_merged}",
                    if frame {
                        div { class: "w-full pointer-events-none", style: "{strip_style}", {registry.render_spec(&tile, None, view)} }
                        for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                        div { class: "w-full pointer-events-none", style: "{strip_style}", {registry.render_spec(&tile, None, view)} }
                    } else {
                        div { class: "w-full h-full", style: "{layer_style}", {registry.render_spec(&tile, None, view)} }
                    }
                }
            }
        }
    }
}
