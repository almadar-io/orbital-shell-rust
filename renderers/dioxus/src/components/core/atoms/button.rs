//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    merge_class_name, passthrough_attrs, pattern_spec, DioxusPatternRenderer, DioxusView,
};
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
        let alt = ["name", "category"]
            .iter()
            .find_map(|key| asset.get(key).and_then(Value::as_str))
            .unwrap_or("");
        spec.insert("alt".into(), Value::string(alt));
        spec.insert(
            "className".into(),
            Value::String("flex-shrink-0".to_string().into()),
        );
    } else {
        let fields = props.as_object()?;
        let name = name_keys.iter().find_map(|key| {
            fields
                .get(*key)
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
        })?;
        spec.insert("type".into(), Value::String("icon".to_string().into()));
        spec.insert("name".into(), Value::String(name.to_string().into()));
        spec.insert(
            "className".into(),
            Value::String("h-icon-default w-icon-default".to_string().into()),
        );
    }
    Some(Value::Object(spec.into()))
}

/// The button root classes for `props`, shared with bodies that emit the button element around their own content.
pub(crate) fn button_class(props: &Value) -> String {
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
    let busy = get_bool_or(props, "isLoading", false);
    let cursor = if busy { "cursor-progress" } else { "" };
    let base = format!("relative inline-flex items-center justify-center gap-2 font-medium rounded-interactive interactive-border cursor-pointer {chrome} transition-all duration-normal focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-offset-[length:var(--focus-ring-offset)] disabled:opacity-50 disabled:cursor-not-allowed {variant_class} {size_class} {cursor}");
    merge_class_name(&base, props)
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
        let size = get_string_or(props, "size", "md");
        let disabled = get_bool_or(props, "disabled", false);
        let busy = get_bool_or(props, "isLoading", false);
        let class = button_class(props);
        let style = crate::styles::style_with_props("", props);
        let html_type = get_string_or(props, "htmlType", "button");
        let action = get_string(props, "action");
        let href = get_string(props, "href").filter(|href| !href.is_empty());
        let asset = props
            .as_object()
            .and_then(|fields| fields.get("iconAsset"))
            .filter(|asset| {
                asset
                    .get("url")
                    .and_then(Value::as_str)
                    .is_some_and(|url| !url.is_empty())
            });
        let icon_px = if size == "lg" { 20 } else { 16 };
        let left_icon = icon_spec(props, &["leftIcon", "icon"], asset, icon_px);
        let right_icon = icon_spec(props, &["rightIcon", "iconRight"], None, icon_px);
        let spinner = pattern_spec(
            "icon",
            [
                ("name", Value::string("loader-2")),
                (
                    "className",
                    Value::string("h-icon-default w-icon-default animate-spin"),
                ),
            ],
        );
        let action_payload = props
            .as_object()
            .and_then(|value| value.get("actionPayload"))
            .cloned()
            .unwrap_or_else(|| Value::Object(orbital_core::ValueMap::new().into()));
        let sink = view.sink;
        let navigate = view.navigate;
        let mut defaults: Vec<(&'static str, Value)> = Vec::new();
        if busy {
            defaults.push(("aria-busy", Value::Bool(true)));
        }
        if disabled || busy {
            defaults.push(("aria-disabled", Value::Bool(true)));
        }
        let mut attrs = passthrough_attrs(props, &defaults);
        let has_test_id = props
            .as_object()
            .is_some_and(|fields| fields.contains_key("data-testid"));
        if let (false, Some(action)) = (has_test_id, action.as_ref()) {
            attrs.push(Attribute::new(
                "data-testid",
                format!("action-{action}"),
                None,
                false,
            ));
        }
        let children = child_patterns(props, view.trait_content);
        let has_children = !children.is_empty();
        let label = get_string(props, "label").filter(|label| !label.is_empty() && !has_children);
        let content_class = if busy {
            "inline-flex flex-1 min-w-0 items-center [justify-content:inherit] gap-2 almadar-busy-content"
        } else {
            "inline-flex flex-1 min-w-0 items-center [justify-content:inherit] gap-2"
        };
        let content = rsx! {
            if busy { span { class: "almadar-busy-indicator absolute inset-0 flex items-center justify-center", aria_hidden: "true", {registry.render_spec(&spinner, None, view)} } }
            span { class: "{content_class}",
                if let Some(icon) = left_icon.as_ref() { span { class: "flex-shrink-0", {registry.render_spec(icon, None, view)} } }
                for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                if let Some(label) = label.as_ref() { span { "{label}" } }
                if let Some(icon) = right_icon.as_ref() { span { class: "flex-shrink-0", {registry.render_spec(icon, None, view)} } }
            }
        };
        if let Some(href) = href {
            return rsx! {
                a { class: "{class}", style: "{style}", href: "{href}",
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
                    {content}
                }
            };
        }
        rsx! {
            button {
                class: "{class}",
                style: "{style}",
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
                {content}
            }
        }
    }
}
