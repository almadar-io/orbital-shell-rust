use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_string_or, };
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

pub struct FormActionsRenderer;

fn button_spec(action: &Value, default_variant: &str) -> Value {
    let mut fields = orbital_core::ValueMap::new();
    fields.insert("type".into(), Value::string("button"));
    fields.insert(
        "label".into(),
        Value::string(get_string_or(action, "label", "")),
    );
    fields.insert(
        "variant".into(),
        Value::string(get_string_or(action, "variant", default_variant)),
    );
    fields.insert(
        "htmlType".into(),
        Value::string(
            if action.get("actionType").and_then(Value::as_str) == Some("submit") {
                "submit"
            } else {
                "button"
            },
        ),
    );
    if let Some(event) = action.get("event") {
        fields.insert("action".into(), event.clone());
    }
    if let Some(destination) = action.get("navigatesTo") {
        fields.insert("href".into(), destination.clone());
    }
    Value::Object(fields.into())
}

impl DioxusPatternRenderer for FormActionsRenderer {
    fn pattern_name(&self) -> &str {
        "form-actions"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let children = child_patterns(props, view.trait_content);
        let mut buttons = Vec::new();
        if children.is_empty() {
            if let Some(filters) = props
                .get("filters")
                .and_then(Value::as_array)
                .filter(|filters| !filters.is_empty())
            {
                for filter in filters {
                    if filter.get("label").is_some() {
                        buttons.push(button_spec(filter, "ghost"));
                    }
                }
            } else {
                if let Some(secondary) = props.get("secondary").and_then(Value::as_array) {
                    buttons.extend(secondary.iter().map(|action| button_spec(action, "ghost")));
                }
                if let Some(primary) = props.get("primary") {
                    buttons.push(button_spec(primary, "primary"));
                }
            }
        }
        let variant = get_string_or(props, "variant", "default");
        let orientation = get_string_or(props, "orientation", "horizontal");
        let look = get_string_or(props, "look", "right-aligned-buttons");
        let variant_class = match variant.as_str() {
            "segmented" | "toggle" => "[&>button]:rounded-none [&>button:first-child]:rounded-l-interactive [&>button:last-child]:rounded-r-interactive [&>button:not(:first-child)]:border-l-0",
            _ => "",
        };
        let orientation_class = if orientation == "vertical" {
            "flex-col [&>button:first-child]:rounded-t-interactive [&>button:last-child]:rounded-b-interactive [&>button:not(:first-child)]:border-t-0 [&>button:not(:first-child)]:border-l"
        } else {
            "flex-row"
        };
        let look_class = match look.as_str() {
            "floating-bar" => "fixed bottom-section left-1/2 -translate-x-1/2 shadow-elevation-toast bg-card p-card-sm rounded-container",
            "inline-row" => "gap-2 inline-flex",
            "dropdown-menu" | "command-palette-trigger" => "[&>button:not(:first-child)]:hidden",
            _ => "",
        };
        let class = merge_class_name(
            &format!("inline-flex gap-2 {variant_class} {orientation_class} {look_class}"),
            props,
        );
        rsx! { div { class: "{class}", role: "group", ..shared_root_attrs(props),
            if !children.is_empty() {
                for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
            } else {
                for button in buttons.iter() { {registry.render_spec(button, None, view)} }
            }
        } }
    }
}
