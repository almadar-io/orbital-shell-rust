use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::get_string_or;
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

pub struct PageTransitionRenderer;

impl DioxusPatternRenderer for PageTransitionRenderer {
    fn pattern_name(&self) -> &str {
        "page-transition"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = cn(&[
            "animate-page-in",
            get_string_or(props, "className", "").as_str(),
        ]);
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{class}", "data-page-transition": "owner", ..shared_root_attrs(props),
                for (child, source) in children.iter() {
                    {view.render_child(registry, child, source.as_deref())}
                }
            }
        }
    }
}
