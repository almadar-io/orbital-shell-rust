use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `range-slider` pattern.
pub struct RangeSliderRenderer;

impl RangeSliderRenderer {
    const NAME: &str = "range-slider";
}

impl DioxusPatternRenderer for RangeSliderRenderer {
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
        let class = merge_class_name("relative w-full", props);
        let min = get_number_or(props, "min", 0.0);
        let max = get_number_or(props, "max", 100.0);
        let value = get_number_or(props, "value", min);
        let step = get_number_or(props, "step", 1.0);
        let disabled = get_bool_or(props, "disabled", false);
        let show_ticks = get_bool_or(props, "showTicks", false);
        let show_tooltip = get_bool_or(props, "showTooltip", false);
        let size = get_string_or(props, "size", "md");
        let track_size = match size.as_str() {
            "sm" => "h-1",
            "lg" => "h-2",
            _ => "h-1.5",
        };
        let thumb_size = match size.as_str() {
            "sm" => "w-3 h-3",
            "lg" => "w-5 h-5",
            _ => "w-4 h-4",
        };
        let percent = if max > min {
            ((value - min) / (max - min) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let buffered = get_number(props, "buffered").map(|amount| amount.clamp(0.0, 100.0));
        let tick_count = if show_ticks && step.is_finite() && step > 0.0 && max > min {
            (((max - min) / step).floor() as usize).min(20)
        } else {
            0
        };
        let action = get_string(props, "action");
        let action_payload = props
            .as_object()
            .and_then(|fields| fields.get("actionPayload"))
            .and_then(Value::as_object)
            .cloned();
        let sink = view.sink;
        rsx! { div { class: "{class}",
            div { class: "relative w-full py-2 group",
                div { class: "absolute inset-x-0 rounded-full bg-muted {track_size}", style: "top:50%;transform:translateY(-50%)" }
                if let Some(buffered) = buffered {
                    div { class: "absolute rounded-full bg-muted-foreground opacity-30 {track_size}", style: "top:50%;transform:translateY(-50%);width:{buffered}%" }
                }
                div { class: "absolute rounded-full bg-primary {track_size}", style: "top:50%;transform:translateY(-50%);width:{percent}%" }
                input {
                    class: "absolute inset-0 w-full opacity-0 cursor-pointer disabled:cursor-not-allowed",
                    r#type: "range", min: "{min}", max: "{max}", step: "{step}", value: "{value}", disabled: disabled,
                    aria_valuemin: "{min}", aria_valuemax: "{max}", aria_valuenow: "{value}", aria_valuetext: "{value}",
                    oninput: move |event| {
                        if disabled { return; }
                        if let (Some(sink), Some(action)) = (sink, &action) {
                            if let Ok(number) = event.value().parse::<f64>() {
                                let mut payload = action_payload.clone().unwrap_or_default();
                                payload.insert("value".into(), Value::Number(number));
                                sink.call((action.clone(), Value::Object(payload.into())));
                            }
                        }
                    },
                    ..shared_root_attrs(props),
                }
                div { class: "absolute rounded-full bg-primary-foreground border-[length:var(--border-width)] border-primary shadow-elevation-interactive pointer-events-none {thumb_size}", style: "top:50%;left:{percent}%;transform:translate(-50%,-50%)" }
                if show_tooltip {
                    div { class: "hidden group-focus-within:block group-active:block absolute -top-8 px-2 py-0.5 rounded-container bg-foreground text-background text-xs font-medium whitespace-nowrap pointer-events-none", style: "left:{percent}%;transform:translateX(-50%)", "{value}" }
                }
            }
            if tick_count > 0 {
                div { class: "relative w-full h-2 mt-1",
                    for index in 0..=tick_count {
                        div { key: "{index}", class: "absolute w-px h-1.5 bg-muted-foreground", style: format!("left:{}%", index as f64 / tick_count as f64 * 100.0) }
                    }
                }
            }
        } }
    }
}
