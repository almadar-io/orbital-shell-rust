//! Native Dioxus renderer for the `side-panel` pattern, mirroring the web `SidePanel` molecule.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, typography_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `side-panel` pattern.
pub struct SidePanelRenderer;

impl SidePanelRenderer {
    const NAME: &str = "side-panel";
}

impl DioxusPatternRenderer for SidePanelRenderer {
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
        if !get_bool_or(props, "isOpen", false) {
            return rsx! {};
        }
        let title = get_string_or(props, "title", "");
        let width = get_string_or(props, "width", "w-full sm:w-96");
        let left = get_string_or(props, "position", "right") == "left";
        let close_event = get_string(props, "closeEvent");
        let close_label = "Close panel";

        let overlay = if get_bool_or(props, "showOverlay", true) {
            let mut fields = vec![(
                "className",
                Value::string("fixed inset-0 bg-background/80 backdrop-blur-sm z-40 lg:hidden"),
            )];
            if let Some(event) = &close_event {
                fields.push(("action", Value::string(event.to_string())));
            }
            Some(pattern_spec("box", fields))
        } else {
            None
        };

        let mut button_fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("x")),
            ("label", Value::string("")),
            ("aria-label", Value::string(close_label)),
        ];
        if let Some(event) = &close_event {
            button_fields.push(("action", Value::string(event.to_string())));
        }
        let title_spec = typography_spec("h6", None, "", &title);
        let button_spec = pattern_spec("button", button_fields);
        let box_base = "bg-transparent rounded-none shadow-none";
        let header_class = cn(&[
            box_base,
            "flex items-center justify-between p-4 border-b-heavy border-border sticky top-0 bg-card z-10",
        ]);
        let content_class = cn(&[box_base, "p-4 flex-1 overflow-y-auto"]);
        let children = child_patterns(props, view.trait_content);
        let aside_class = merge_class_name(
            &cn(&[
                "fixed top-16 lg:top-0 bottom-0 z-[60]",
                "bg-card surface-material",
                "border-l-heavy border-border",
                if left {
                    "border-l-0 border-r-heavy"
                } else {
                    ""
                },
                "flex flex-col",
                &width,
                if left { "left-0" } else { "right-0" },
            ]),
            props,
        );
        rsx! {
            if let Some(overlay) = overlay { {registry.render_spec(&overlay, None, view)} }
            aside { class: "{aside_class}", ..shared_root_attrs(props),
                div { class: "{header_class}",
                    {registry.render_spec(&title_spec, None, view)}
                    {registry.render_spec(&button_spec, None, view)}
                }
                div { class: "{content_class}",
                    for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                }
            }
        }
    }
}
