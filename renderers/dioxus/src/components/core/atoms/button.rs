//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `button` pattern.
pub struct ButtonRenderer;

fn icon_spec(props: &Value, name_keys: &[&str], asset: Option<&Value>, px: i64) -> Option<Value> {
    let mut spec = orbital_core::ValueMap::new();
    if let Some(asset) = asset {
        spec.insert(
            "type".into(),
            Value::String("atlas-image".to_string().into()),
        );
        spec.insert("asset".into(), asset.clone());
        spec.insert("size".into(), Value::Int(px));
    } else {
        let fields = props.as_object()?;
        let name = name_keys
            .iter()
            .find_map(|key| fields.get(*key).and_then(Value::as_str))?;
        spec.insert("type".into(), Value::String("icon".to_string().into()));
        spec.insert("name".into(), Value::String(name.to_string().into()));
    }
    spec.insert(
        "className".into(),
        Value::String(
            "flex-shrink-0 h-icon-default w-icon-default"
                .to_string()
                .into(),
        ),
    );
    Some(Value::Object(spec.into()))
}

impl ButtonRenderer {
    const NAME: &str = "button";
}

impl DioxusPatternRenderer for ButtonRenderer {
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
        let variant = get_string_or(props, "variant", "primary");
        let size = get_string_or(props, "size", "md");
        let variant_class = match variant.as_str() {
            "secondary" => "bg-transparent text-foreground border border-accent hover:bg-accent hover:text-accent-foreground hover:border-accent active:scale-[var(--active-scale)]",
            "ghost" => "bg-transparent text-muted-foreground border border-transparent hover:text-primary-foreground hover:bg-primary hover:border-primary active:scale-[var(--active-scale)]",
            "danger" | "destructive" => "bg-surface text-foreground border-[length:var(--border-width)] border-error shadow-elevation-interactive hover:bg-error hover:text-error-foreground hover:shadow-elevation-popover active:scale-[var(--active-scale)] active:shadow-elevation-pressed",
            "success" => "bg-surface text-foreground border-[length:var(--border-width)] border-success shadow-elevation-interactive hover:bg-success hover:text-success-foreground hover:shadow-elevation-popover active:scale-[var(--active-scale)] active:shadow-elevation-pressed",
            "warning" => "bg-surface text-foreground border-[length:var(--border-width)] border-warning shadow-elevation-interactive hover:bg-warning hover:text-warning-foreground hover:shadow-elevation-popover active:scale-[var(--active-scale)] active:shadow-elevation-pressed",
            "link" => "bg-transparent text-muted-foreground border-none shadow-none underline-offset-4 hover:text-foreground hover:underline",
            "default" => "bg-secondary text-secondary-foreground border-[length:var(--border-width-thin)] border-border hover:bg-secondary-hover active:scale-[var(--active-scale)]",
            _ => "bg-primary text-primary-foreground border-none shadow-elevation-interactive hover:bg-primary-hover hover:shadow-elevation-popover active:scale-[var(--active-scale)] active:shadow-elevation-pressed",
        };
        let size_class = if variant == "link" {
            "h-auto px-0 text-sm"
        } else {
            match size.as_str() {
                "sm" => "h-button-sm px-3 text-sm",
                "lg" => "h-button-lg px-6 text-base",
                _ => "h-button-md px-4 text-sm",
            }
        };
        let chrome = if variant == "link" {
            ""
        } else {
            "chrome-button"
        };
        let base = format!("relative inline-flex items-center justify-center gap-2 font-medium rounded-interactive interactive-border cursor-pointer transition-all duration-normal focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-offset-[length:var(--focus-ring-offset)] disabled:opacity-50 disabled:cursor-not-allowed {chrome} {variant_class} {size_class}");
        let class = merge_class_name(&base, props);
        let label = get_string_or(props, "label", "Button");
        let html_type = get_string_or(props, "htmlType", "button");
        let action = get_string(props, "action");
        let disabled = get_bool_or(props, "disabled", false);
        let busy = get_bool_or(props, "isLoading", false);
        let href = get_string(props, "href").filter(|href| !href.is_empty());
        let asset = props.as_object().and_then(|fields| fields.get("iconAsset"));
        let icon_px = if size == "lg" { 20 } else { 16 };
        let left_icon = icon_spec(props, &["leftIcon", "icon"], asset, icon_px);
        let right_icon = icon_spec(props, &["rightIcon", "iconRight"], None, icon_px);
        let mut spinner_fields = orbital_core::ValueMap::new();
        spinner_fields.insert("type".into(), Value::String("spinner".to_string().into()));
        spinner_fields.insert("size".into(), Value::String("sm".to_string().into()));
        let spinner = Value::Object(spinner_fields.into());
        let action_payload = props
            .as_object()
            .and_then(|value| value.get("actionPayload"))
            .cloned()
            .unwrap_or_else(|| Value::Object(orbital_core::ValueMap::new().into()));
        let sink = view.sink;
        let navigate = view.navigate;
        let attrs = shared_root_attrs_with_defaults(
            props,
            &[
                ("aria-busy", Value::Bool(busy)),
                ("aria-disabled", Value::Bool(disabled || busy)),
            ],
        );
        if let Some(href) = href {
            return rsx! {
                a { class: "{class}", href: "{href}",
                    onclick: move |event| {
                        if disabled || busy { event.prevent_default(); return; }
                        if let (Some(sink), Some(action)) = (sink, &action) {
                            sink.call((action.clone(), action_payload.clone()));
                        }
                        if href.starts_with('/') || href.starts_with('#') {
                            if let Some(navigate) = navigate {
                                event.prevent_default();
                                navigate.call(href.clone());
                            }
                        }
                    },
                    ..attrs,
                    if busy { span { class: "almadar-busy-indicator absolute inset-0 flex items-center justify-center", {registry.render_spec(&spinner, None, view)} } }
                    span { class: "inline-flex flex-1 min-w-0 items-center gap-2",
                        if let Some(icon) = left_icon.as_ref() { {registry.render_spec(icon, None, view)} }
                        span { "{label}" }
                        if let Some(icon) = right_icon.as_ref() { {registry.render_spec(icon, None, view)} }
                    }
                }
            };
        }
        rsx! {
            button {
                class: "{class}",
                r#type: "{html_type}",
                disabled: disabled,
                onclick: move |_| {
                    if !disabled && !busy {
                        if let (Some(s), Some(action)) = (sink, &action) {
                            s.call((action.clone(), action_payload.clone()));
                        }
                    }
                },
                ..attrs,
                if busy { span { class: "almadar-busy-indicator absolute inset-0 flex items-center justify-center", {registry.render_spec(&spinner, None, view)} } }
                span { class: "inline-flex flex-1 min-w-0 items-center gap-2",
                    if let Some(icon) = left_icon.as_ref() { {registry.render_spec(icon, None, view)} }
                    span { "{label}" }
                    if let Some(icon) = right_icon.as_ref() { {registry.render_spec(icon, None, view)} }
                }
            }
        }
    }
}
