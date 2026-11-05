//! Native Dioxus body for the `number-stepper` pattern, mirroring `NumberStepper.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{number_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct NumberStepperRenderer;

impl NumberStepperRenderer {
    const NAME: &str = "number-stepper";
}

fn js_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        number.to_string()
    }
}

impl DioxusPatternRenderer for NumberStepperRenderer {
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
        let (button, text, icon_size) = match size.as_str() {
            "sm" => ("w-7 h-7", "text-sm min-w-[2rem]", "xs"),
            "lg" => ("w-11 h-11", "text-lg min-w-[3rem]", "md"),
            _ => ("w-9 h-9", "text-base min-w-[2.5rem]", "sm"),
        };
        let value = get_number_or(props, "value", 0.0);
        let min = get_number(props, "min");
        let max = get_number(props, "max");
        let step = get_number_or(props, "step", 1.0);
        let disabled = get_bool_or(props, "disabled", false);
        let at_min = min.is_some_and(|min| value <= min);
        let at_max = max.is_some_and(|max| value >= max);
        let class = merge_class_name(
            &format!(
                "inline-flex items-center rounded-interactive border-[length:var(--border-width)] border-border bg-surface {}",
                if disabled { "opacity-50 cursor-not-allowed" } else { "" }
            ),
            props,
        );
        let label = get_string(props, "aria-label")
            .or_else(|| get_string(props, "label"))
            .unwrap_or_else(|| "Number stepper".to_string());
        let button_base = "inline-flex items-center justify-center text-foreground hover:bg-muted active:bg-muted transition-colors duration-instant disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset";
        let minus_class = format!("{button_base} rounded-l-interactive {button}");
        let plus_class = format!("{button_base} rounded-r-interactive {button}");
        let value_class = format!("text-center font-medium tabular-nums text-foreground border-x-[length:var(--border-width)] border-border px-1 select-none {text}");
        let minus_icon = pattern_spec(
            "icon",
            [
                ("name", Value::string("minus")),
                ("size", Value::string(icon_size)),
            ],
        );
        let plus_icon = pattern_spec(
            "icon",
            [
                ("name", Value::string("plus")),
                ("size", Value::string(icon_size)),
            ],
        );
        let action = get_string(props, "action");
        let payload = props
            .as_object()
            .and_then(|fields| fields.get("actionPayload"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_else(ValueMap::new);
        let sink = view.sink;
        let emit = move |target: f64| {
            let (Some(sink), Some(action)) = (sink, &action) else {
                return;
            };
            let stepped = ((target / step) + 0.5).floor() * step;
            let clamped = stepped
                .min(max.unwrap_or(f64::INFINITY))
                .max(min.unwrap_or(f64::NEG_INFINITY));
            let mut fields = payload.clone();
            fields.insert("value".into(), number_value(clamped));
            sink.call((action.clone(), Value::Object(fields.into())));
        };
        let emit_plus = emit.clone();
        let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
            .into_iter()
            .filter(|attr| !matches!(attr.name, "role" | "aria-label"))
            .collect();
        let shown = js_number(value);
        rsx! {
            div { class: "{class}", role: "group", "aria-label": "{label}", ..attrs,
                button {
                    r#type: "button",
                    class: "{minus_class}",
                    disabled: disabled || at_min,
                    "aria-label": "Decrease",
                    onclick: move |_| { if !at_min { emit(value - step); } },
                    {registry.render_spec(&minus_icon, None, view)}
                }
                span { class: "{value_class}", "aria-live": "polite", "aria-atomic": "true", "{shown}" }
                button {
                    r#type: "button",
                    class: "{plus_class}",
                    disabled: disabled || at_max,
                    "aria-label": "Increase",
                    onclick: move |_| { if !at_max { emit_plus(value + step); } },
                    {registry.render_spec(&plus_icon, None, view)}
                }
            }
        }
    }
}
