use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number, get_number_or, get_string_or, };
use orbital_core::Value;

pub struct HealthBarRenderer;

impl DioxusPatternRenderer for HealthBarRenderer {
    fn pattern_name(&self) -> &str {
        "health-bar"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let current = get_number_or(props, "current", 3.0);
        let max = get_number_or(props, "max", 5.0);
        let format = get_string_or(props, "format", "hearts");
        let size = get_string_or(props, "size", "md");
        let animated = get_bool_or(props, "animated", true);
        let class = merge_class_name("almadar-health-bar", props);
        let percent = if max > 0.0 {
            (current / max * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let height = match size.as_str() {
            "sm" => 8,
            "lg" => 16,
            _ => 12,
        };
        let frame_asset = props.as_object().and_then(|p| p.get("frameAsset")).cloned();
        let fill_asset = props.as_object().and_then(|p| p.get("fillAsset")).cloned();
        let fill_color = if percent > 66.0 {
            "var(--color-success)"
        } else if percent > 33.0 {
            "var(--color-warning)"
        } else {
            "var(--color-error)"
        };
        let track_style = format!(
            "position:relative;overflow:hidden;border-radius:9999px;height:{height}px;min-width:96px;background:var(--color-muted);"
        );
        let fill_style = format!(
            "position:absolute;inset:0 auto 0 0;height:100%;width:{percent}%;background:{fill_color};{}",
            if animated {
                "transition:width 300ms;"
            } else {
                ""
            }
        );
        let label = get_string_or(props, "aria-label", &format!("{current} of {max}"));

        match format.as_str() {
            "hearts" => {
                let count = max.max(0.0) as usize;
                let heart_size = match size.as_str() {
                    "sm" => 16,
                    "lg" => 32,
                    _ => 24,
                };
                rsx! {
                    div { class: "{class}", style: "display:flex;align-items:center;gap:4px;",
                        ..shared_root_attrs_with_defaults(props, &[("aria-label", Value::String(label.clone().into()))]),
                        for index in 0..count {
                            {
                                let filled = (index as f64) < current;
                                let color = if filled { "var(--color-error)" } else { "var(--color-muted-foreground)" };
                                let heart = if filled { "♥" } else { "♡" };
                                rsx! { span { style: "font-size:{heart_size}px;color:{color};", "{heart}" } }
                            }
                        }
                    }
                }
            }
            "bar" => rsx! {
                div { class: "{class}", style: "{track_style}",
                    aria_valuenow: current.to_string(), aria_valuemin: "0", aria_valuemax: max.to_string(),
                    ..shared_root_attrs_with_defaults(props, &[("aria-label", Value::String(label.clone().into())), ("role", Value::String("progressbar".into()))]),
                    if let Some(asset) = frame_asset.as_ref() {
                        div { style: "position:absolute;inset:0;", {crate::components::game::asset_panel::fill_parent(asset, "stretch", view, registry)} }
                    }
                    div { style: "{fill_style}",
                        if let Some(asset) = fill_asset.as_ref() {
                            {crate::components::game::asset_panel::fill_parent(asset, "repeat", view, registry)}
                        }
                    }
                }
            },
            "progress" => {
                let level = get_number(props, "level");
                let show_label = get_bool_or(props, "showLabel", true);
                let suffix = get_string_or(props, "labelSuffix", "");
                let value_label = if suffix.is_empty() {
                    format!("{current} / {max}")
                } else {
                    format!("{current} / {max} {suffix}")
                };
                rsx! {
                    div { class: "{class}", style: "display:flex;align-items:center;gap:8px;",
                        ..shared_root_attrs_with_defaults(props, &[("aria-label", Value::String(label.clone().into()))]),
                        if let Some(level) = level {
                            span { class: "almadar-health-bar__level", "Lv.{level}" }
                        }
                        div { style: "display:flex;flex-direction:column;flex:1;",
                            div { role: "progressbar", aria_label: "{label}", style: "{track_style}",
                                aria_valuenow: current.to_string(), aria_valuemin: "0", aria_valuemax: max.to_string(),
                                if let Some(asset) = frame_asset.as_ref() {
                                    div { style: "position:absolute;inset:0;", {crate::components::game::asset_panel::fill_parent(asset, "stretch", view, registry)} }
                                }
                                div { style: "{fill_style}",
                                    if let Some(asset) = fill_asset.as_ref() {
                                        {crate::components::game::asset_panel::fill_parent(asset, "repeat", view, registry)}
                                    }
                                }
                            }
                            if show_label { span { class: "almadar-muted", "{value_label}" } }
                        }
                    }
                }
            }
            _ => rsx! {
                span { class: "{class}", style: "font-family:monospace;font-weight:700;", ..shared_root_attrs_with_defaults(props, &[("aria-label", Value::String(label.into()))]), "{current}/{max}" }
            },
        }
    }
}
