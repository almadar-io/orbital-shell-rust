//! Native Dioxus renderer for the `section` pattern, mirroring the web `Section` molecule.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, typography_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `section` pattern.
pub struct SectionRenderer;

impl SectionRenderer {
    const NAME: &str = "section";
}

impl DioxusPatternRenderer for SectionRenderer {
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
        let padding = match get_string_or(props, "padding", "md").as_str() {
            "none" => "p-0",
            "sm" => "p-3",
            "md" => "p-4",
            "lg" => "p-6",
            "xl" => "p-8",
            _ => "",
        };
        let variant = match get_string_or(props, "variant", "default").as_str() {
            "card" => "surface-content",
            "bordered" => "border-[length:var(--border-width)] border-border rounded-container",
            "filled" => "bg-muted rounded-container",
            _ => "",
        };
        let class = merge_class_name(&cn(&[padding, variant]), props);
        let title = get_string(props, "title").filter(|text| !text.is_empty());
        let description = get_string(props, "description").filter(|text| !text.is_empty());
        let action = props
            .as_object()
            .and_then(|fields| fields.get("action"))
            .filter(|value| !value.is_null())
            .cloned();
        let divider = get_bool_or(props, "divider", false);
        let header_class = get_string_or(props, "headerClassName", "");
        let content_class = get_string_or(props, "contentClassName", "");

        let has_header = title.is_some() || description.is_some() || action.is_some();
        let title_spec = title
            .as_ref()
            .map(|title| typography_spec("h4", None, "text-foreground", title));
        let description_spec = description
            .as_ref()
            .map(|text| typography_spec("body", Some("muted"), "mt-1", text));
        let box_base = "bg-transparent rounded-none shadow-none";
        let header_class = cn(&[
            box_base,
            "flex flex-col sm:flex-row sm:items-start sm:justify-between gap-2 sm:gap-4",
            if divider {
                "pb-4 mb-4 border-b-[length:var(--border-width)] border-border"
            } else {
                "mb-4"
            },
            &header_class,
        ]);
        let text_class = cn(&[box_base, "flex-1 min-w-0"]);
        let action_class = cn(&[box_base, "flex-shrink-0 flex items-center gap-2"]);
        let content_class = cn(&[box_base, &content_class]);
        let children = child_patterns(props, view.trait_content);
        rsx! { section { class: "{class}", ..shared_root_attrs(props),
            if has_header {
                div { class: "{header_class}",
                    div { class: "{text_class}",
                        if let Some(spec) = title_spec { {registry.render_spec(&spec, None, view)} }
                        if let Some(spec) = description_spec { {registry.render_spec(&spec, None, view)} }
                    }
                    if let Some(action) = action { div { class: "{action_class}", {view.render_node(registry, &action)} } }
                }
            }
            div { class: "{content_class}",
                for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
            }
        } }
    }
}
