//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `avatar` pattern.
pub struct AvatarRenderer;

#[component]
fn AvatarVisual(
    src: String,
    alt: String,
    initials: String,
    use_initials: bool,
    icon_glyph: String,
) -> Element {
    let mut image_failed = use_signal(|| false);
    if !src.is_empty() && !image_failed() {
        rsx! { img {
            class: "w-full h-full object-cover",
            src: "{src}",
            alt: "{alt}",
            onerror: move |_| image_failed.set(true),
        } }
    } else if use_initials {
        rsx! { div {
            class: "w-full h-full flex items-center justify-center font-bold bg-primary text-primary-foreground",
            "{initials}"
        } }
    } else {
        rsx! { span { class: "text-foreground", "{icon_glyph}" } }
    }
}

impl AvatarRenderer {
    const NAME: &str = "avatar";
}

impl DioxusPatternRenderer for AvatarRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let size = get_string_or(props, "size", "md");
        let dimensions = match size.as_str() {
            "xs" => "w-6 h-6 text-xs",
            "sm" => "w-8 h-8 text-sm",
            "lg" => "w-12 h-12 text-lg",
            "xl" => "w-16 h-16 text-xl",
            _ => "w-10 h-10 text-base",
        };
        let status_dimensions = match size.as_str() {
            "xs" => "w-1.5 h-1.5",
            "sm" => "w-2 h-2",
            "lg" => "w-3 h-3",
            "xl" => "w-4 h-4",
            _ => "w-2.5 h-2.5",
        };
        let badge_dimensions = match size.as_str() {
            "xs" => "w-3 h-3 text-xs",
            "sm" => "w-4 h-4 text-xs",
            "lg" => "w-6 h-6 text-sm",
            "xl" => "w-7 h-7 text-base",
            _ => "w-5 h-5 text-xs",
        };
        let name = get_string_or(props, "name", "");
        let explicit_initials = get_string_or(props, "initials", "");
        let derived_initials = if name.is_empty() {
            String::new()
        } else {
            let mut parts = name.split_whitespace();
            let first = parts.next().unwrap_or("");
            let last = parts.last();
            match last {
                Some(last) => format!(
                    "{}{}",
                    first.chars().next().unwrap_or(' '),
                    last.chars().next().unwrap_or(' ')
                ),
                None => first.chars().take(2).collect(),
            }
            .to_uppercase()
        };
        let initials = if explicit_initials.is_empty() {
            derived_initials
        } else {
            explicit_initials
        };
        let src = get_string_or(props, "src", "");
        let alt = get_string_or(props, "alt", &name);
        let icon = get_string_or(props, "icon", "");
        let icon_glyph = icon_name_to_glyph(if icon.is_empty() { "user" } else { &icon });
        let status = get_string(props, "status");
        let status_label = status.as_deref().unwrap_or("").to_string();
        let status_color = status
            .as_deref()
            .map(super::status_dot::color_class)
            .unwrap_or("");
        let badge = props
            .as_object()
            .and_then(|object| object.get("badge"))
            .map(|value| match value {
                Value::Int(n) if *n > 99 => "99+".to_string(),
                Value::Int(n) => n.to_string(),
                Value::Number(n) if *n > 99.0 => "99+".to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            });
        let use_initials = !initials.is_empty()
            && (icon.is_empty() || !get_string_or(props, "initials", "").is_empty());
        let action = get_string(props, "action");
        let payload = props
            .as_object()
            .and_then(|object| object.get("actionPayload"))
            .cloned()
            .unwrap_or(Value::Null);
        let sink = view.sink;
        let base = format!("relative inline-flex items-center justify-center bg-muted border-[length:var(--border-width)] border-border overflow-hidden {dimensions}");
        let class = merge_class_name(&base, props);
        rsx! { div { class: "relative inline-block", ..crate::registry::shared_root_attrs(props),
            div { class: "{class}", aria_label: "{alt}",
                onclick: move |_| { if let (Some(sink), Some(action)) = (sink, &action) { sink.call((action.clone(), payload.clone())); } },
                AvatarVisual { key: "{src}", src, alt: alt.clone(), initials, use_initials, icon_glyph: icon_glyph.to_string() }
            }
            if status.is_some() { span { class: "absolute bottom-0 right-0 border-heavy border-card {status_color} {status_dimensions}", role: "img", aria_label: "{status_label}" } }
            if let Some(badge) = badge { span { class: "absolute -top-1 -right-1 flex items-center justify-center bg-primary text-primary-foreground font-bold border-heavy border-card {badge_dimensions}", "{badge}" } }
        } }
    }
}
