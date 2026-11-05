//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{emit_empty_event, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `badge` pattern.
pub struct BadgeRenderer;

impl BadgeRenderer {
    const NAME: &str = "badge";
}

impl DioxusPatternRenderer for BadgeRenderer {
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
        let variant = get_string_or(props, "variant", "default");
        let size = get_string_or(props, "size", "sm");
        let variant_class = match variant.as_str() {
            "primary" => "bg-primary text-primary-foreground",
            "secondary" => "bg-secondary text-secondary-foreground",
            "success" => "bg-success/10 text-foreground border-[length:var(--border-width)] border-success",
            "warning" => "bg-warning/10 text-foreground border-[length:var(--border-width)] border-warning",
            "danger" | "error" => "bg-error/10 text-foreground border-[length:var(--border-width)] border-error",
            "info" => "bg-info/10 text-foreground border-[length:var(--border-width)] border-info",
            "neutral" => "bg-muted text-muted-foreground border-[length:var(--border-width-thin)] border-border",
            _ => "bg-muted text-foreground border-[length:var(--border-width-thin)] border-border",
        };
        let size_class = match size.as_str() {
            "md" => "px-2.5 py-1 text-sm",
            "lg" => "px-3 py-1.5 text-base",
            _ => "px-2 py-0.5 text-xs",
        };
        let base = format!(
            "inline-flex items-center gap-1 font-bold rounded-pill {variant_class} {size_class}"
        );
        let class = merge_class_name(&base, props);
        let label = props
            .as_object()
            .and_then(|fields| fields.get("label"))
            .map(|value| match value {
                Value::Int(number) => number.to_string(),
                Value::Number(number) => number.to_string(),
                Value::String(text) => text.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let amount = props
            .as_object()
            .and_then(|fields| fields.get("amount"))
            .and_then(|value| match value {
                Value::Int(number) => Some(number.to_string()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            });
        let text = match amount {
            Some(amount) if !label.is_empty() => format!("{label} {amount}"),
            Some(amount) => amount,
            None => label,
        };
        let icon_spec = props.as_object().and_then(|fields| {
            if let Some(asset) = fields.get("iconAsset") {
                let mut spec = orbital_core::ValueMap::new();
                spec.insert(
                    "type".into(),
                    Value::String("atlas-image".to_string().into()),
                );
                spec.insert("asset".into(), asset.clone());
                spec.insert(
                    "size".into(),
                    Value::Int(if size == "lg" { 20 } else { 16 }),
                );
                spec.insert(
                    "className".into(),
                    Value::String("flex-shrink-0".to_string().into()),
                );
                return Some(Value::Object(spec.into()));
            }
            let icon = fields.get("icon")?.as_str()?;
            let mut spec = orbital_core::ValueMap::new();
            spec.insert("type".into(), Value::String("icon".to_string().into()));
            spec.insert("name".into(), Value::String(icon.to_string().into()));
            spec.insert(
                "className".into(),
                Value::String("h-icon-default w-icon-default".to_string().into()),
            );
            Some(Value::Object(spec.into()))
        });
        let on_remove = get_string(props, "onRemove").filter(|name| !name.is_empty());
        if text.is_empty() && icon_spec.is_none() && on_remove.is_none() {
            return rsx! {};
        }
        let remove_label = get_string(props, "removeLabel").unwrap_or_else(|| {
            if text.is_empty() {
                "Remove".to_string()
            } else {
                format!("Remove: {text}")
            }
        });
        let remove_icon = Value::Object({
            let mut spec = orbital_core::ValueMap::new();
            spec.insert("type".into(), Value::string("icon"));
            spec.insert("name".into(), Value::string("x"));
            spec.insert("size".into(), Value::string("xs"));
            spec.into()
        });
        let remove_size = match size.as_str() {
            "md" => "w-5 h-5 ml-1",
            "lg" => "w-6 h-6 ml-1",
            _ => "w-4 h-4 ml-0.5",
        };
        let sink = view.sink;
        rsx! { span { class: "{class}", ..shared_root_attrs(props),
            if let Some(icon) = icon_spec.as_ref() { {registry.render_spec(icon, None, view)} }
            "{text}"
            if on_remove.is_some() {
                button { type: "button", class: "inline-flex items-center justify-center rounded-pill hover:bg-foreground/10 focus:outline-none focus-visible:ring-1 focus-visible:ring-ring transition-colors {remove_size}", aria_label: "{remove_label}",
                    onclick: move |event| {
                        event.stop_propagation();
                        emit_empty_event(sink, on_remove.as_deref());
                    },
                    {registry.render_spec(&remove_icon, None, view)}
                }
            }
        } }
    }
}
