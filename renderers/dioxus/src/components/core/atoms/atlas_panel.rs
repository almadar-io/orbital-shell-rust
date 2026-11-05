//! Dioxus renderer for the shared `atlas-panel` pattern.

use crate::renderer::merge_class_name;
use crate::registry::shared_root_attrs;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `atlas-panel` pattern.
pub struct AtlasPanelRenderer;

impl AtlasPanelRenderer {
    const NAME: &str = "atlas-panel";
}

impl DioxusPatternRenderer for AtlasPanelRenderer {
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
        let children = child_patterns(props, view.trait_content);
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        {
            if children.is_empty() {
                return crate::game::atlas_panel(props, view, false);
            }
            let class = merge_class_name("inline-block relative", props);
            let border = if get_string_or(props, "mode", "nineSlice") == "repeat" {
                0.0
            } else {
                get_number_or(props, "borderWidth", 16.0)
            };
            let style = style_with_props(
                &format!("position:relative;display:inline-block;padding:{border}px"),
                props,
            );
            let skin = crate::game::atlas_panel(props, view, true);
            return rsx! {
                span { class: "{class}", style: "{style}", ..shared_root_attrs(props),
                    span { style: "position:absolute;inset:0;pointer-events:none", {skin} }
                    for (child, src) in children.iter() {
                        {view.render_child(registry, child, src.as_deref())}
                    }
                }
            };
        }
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        {
            let class = merge_class_name("inline-block", props);
            let style = style_with_props("", props);
            rsx! {
                span { class: "{class}", style: "{style}", ..shared_root_attrs(props),
                    for (child, src) in children.iter() {
                        {view.render_child(registry, child, src.as_deref())}
                    }
                }
            }
        }
    }
}
