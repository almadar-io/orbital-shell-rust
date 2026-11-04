//! Dioxus renderer for the shared `atlas-image` pattern.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `atlas-image` pattern.
pub struct AtlasImageRenderer;

impl AtlasImageRenderer {
    const NAME: &str = "atlas-image";
}

impl DioxusPatternRenderer for AtlasImageRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        {
            let image = crate::game::atlas_image(props, view);
            if get_bool_or(props, "fill", false) {
                rsx! { span { style: "position:absolute;inset:0", {image} } }
            } else {
                image
            }
        }
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        {
            let _ = view;
            let class = merge_class_name("inline-block flex-shrink-0", props);
            let asset = props
                .as_object()
                .and_then(|fields| fields.get("asset"))
                .and_then(Value::as_object);
            let url = asset
                .and_then(|fields| fields.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let alt = get_string_or(
                props,
                "alt",
                asset
                    .and_then(|fields| fields.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            let size = get_number(props, "size");
            let width = get_number(props, "width").or(size);
            let height = get_number(props, "height").or(size);
            let fill = get_bool_or(props, "fill", false);
            let fit = get_string_or(props, "fit", "contain");
            let width_style = if fill {
                "width:100%;".to_string()
            } else {
                width.map(|n| format!("width:{n}px;")).unwrap_or_default()
            };
            let height_style = if fill {
                "height:100%;".to_string()
            } else {
                height.map(|n| format!("height:{n}px;")).unwrap_or_default()
            };
            let position_style = if fill {
                "position:absolute;inset:0;"
            } else {
                ""
            };
            let style = format!(
                "{position_style}{width_style}{height_style}object-fit:{fit};image-rendering:pixelated"
            );
            if url.is_empty() {
                return rsx! {};
            }
            if asset.is_some_and(|fields| {
                fields.get("atlas").is_some() && fields.get("sprite").is_some()
            }) {
                return rsx! { span { class: "{class}", style: "{style}" } };
            }
            rsx! { img { class: "{class}", src: "{url}", alt: "{alt}", style: "{style}" } }
        }
    }
}
