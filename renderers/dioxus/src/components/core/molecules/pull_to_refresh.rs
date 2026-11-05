use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number_or, get_string_or};
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct PullToRefreshRenderer;

impl DioxusPatternRenderer for PullToRefreshRenderer {
    fn pattern_name(&self) -> &str {
        "pull-to-refresh"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let threshold = get_number_or(props, "threshold", 60.0);
        let root = cn(&[
            BOX,
            "overflow-hidden relative",
            &cn(&["w-full", get_string_or(props, "className", "").as_str()]),
        ]);
        let indicator = cn(&[
            BOX,
            "flex absolute",
            "top-0 left-0 right-0 z-10 items-center justify-center transition-opacity duration-fast opacity-0",
        ]);
        let indicator_style = format!(
            "height:{threshold}px;transform:translateY(-{threshold}px);pointer-events:none"
        );
        let mark = cn(&[BOX, "transition-transform duration-fast"]);
        let content = cn(&[BOX, "w-full", "min-h-0"]);
        let spinner = pattern_spec(
            "spinner",
            [
                ("size", Value::string("sm")),
                ("className", Value::string("text-primary")),
            ],
        );
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{root}", ..shared_root_attrs(props),
                div { class: "{indicator}", style: "{indicator_style}",
                    div { class: "{mark}", style: "transform:scale(0);opacity:0",
                        {registry.render_spec(&spinner, None, view)}
                    }
                }
                div { class: "{content}",
                    for (child, source) in children.iter() {
                        {view.render_child(registry, child, source.as_deref())}
                    }
                }
            }
        }
    }
}
