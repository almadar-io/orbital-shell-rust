//! Native Dioxus renderer for the `scaled-diagram` pattern: the web's fixed-size wrapper (the measured scale is browser-only).

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{merge_class_name, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `scaled-diagram` pattern.
pub struct ScaledDiagramRenderer;

impl ScaledDiagramRenderer {
    const NAME: &str = "scaled-diagram";
}

impl DioxusPatternRenderer for ScaledDiagramRenderer {
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
        let class = merge_class_name("bg-transparent rounded-none shadow-none w-full", props);
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                div { class: "bg-transparent rounded-none shadow-none", style: "width:max-content;transform-origin:top left",
                    for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                }
            }
        }
    }
}
