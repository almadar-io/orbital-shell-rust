//! Dioxus renderer for the `counter-template` pattern: the web's minimal / standard / full counter composed over layout and button patterns.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, pattern_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `counter-template` pattern.
pub struct CounterTemplateRenderer;

impl CounterTemplateRenderer {
    const NAME: &str = "counter-template";
}

fn s(value: &str) -> Value {
    Value::string(value)
}

fn count_text(entity: &Value) -> String {
    match entity.get("count").and_then(Value::as_number) {
        Some(number) => number.to_string(),
        None => String::new(),
    }
}

fn button(
    entity: &Value,
    props: &Value,
    variant: &str,
    size: &str,
    event_key: &str,
    disabled_key: &str,
    label_key: Option<&str>,
    icon: &str,
    fallback_label: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("variant", s(variant)),
        ("size", s(size)),
        ("icon", s(icon)),
    ];
    if let Some(event) = get_string(props, event_key) {
        fields.push(("action", Value::string(event)));
    }
    let callback = match event_key {
        "decrementEvent" => "onDecrement",
        "incrementEvent" => "onIncrement",
        _ => "onReset",
    };
    if let Some(handler) = props.get(callback).filter(|value| !value.is_null()) {
        fields.push(("onClick", handler.clone()));
    }
    if let Some(disabled) = entity.get(disabled_key).filter(|value| !value.is_null()) {
        fields.push(("disabled", disabled.clone()));
    }
    let label = label_key
        .and_then(|key| entity.get(key).and_then(Value::as_str).map(str::to_string))
        .or_else(|| fallback_label.map(str::to_string));
    if let Some(label) = label {
        fields.push(("label", Value::string(label)));
    }
    pattern_spec("button", fields)
}

fn text(variant: &str, class: &str, color: Option<&str>, content: String) -> Value {
    let mut fields = vec![("variant", s(variant)), ("content", Value::string(content))];
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    if let Some(color) = color {
        fields.push(("color", s(color)));
    }
    pattern_spec("typography", fields)
}

impl DioxusPatternRenderer for CounterTemplateRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let entity = props
            .get("entity")
            .filter(|value| !value.is_null())
            .or(entity)
            .filter(|value| value.as_object().is_some());
        let Some(entity) = entity else {
            return rsx! {};
        };
        let size = get_string_or(props, "size", "md");
        let (display, button_size) = match size.as_str() {
            "sm" => ("text-4xl", "sm"),
            "lg" => ("text-8xl", "lg"),
            _ => ("text-6xl", "md"),
        };
        let class_name = get_string_or(props, "className", "");
        let show_reset = get_bool_or(props, "showReset", true);
        let variant = get_string(props, "variant");
        let title = get_string_or(props, "title", "Counter");
        let count = count_text(entity);
        let mut root: Vec<(String, Value)> = dom_passthrough(props);

        let spec = match variant.as_deref() {
            Some("minimal") => {
                root.push(("gap".to_string(), s("lg")));
                root.push(("align".to_string(), s("center")));
                root.push(("justify".to_string(), s("center")));
                if !class_name.is_empty() {
                    root.push(("className".to_string(), Value::string(class_name)));
                }
                root.push((
                    "children".to_string(),
                    array_value(vec![
                        button(
                            entity,
                            props,
                            "secondary",
                            button_size,
                            "decrementEvent",
                            "decrementDisabled",
                            Some("decrementLabel"),
                            "minus",
                            None,
                        ),
                        text(
                            "h1",
                            &cn(&[display, "tabular-nums min-w-[3ch] text-center"]),
                            None,
                            count,
                        ),
                        button(
                            entity,
                            props,
                            "secondary",
                            button_size,
                            "incrementEvent",
                            "incrementDisabled",
                            Some("incrementLabel"),
                            "plus",
                            None,
                        ),
                    ]),
                ));
                pattern_spec("hstack", root)
            }
            other => {
                let full = other == Some("full");
                let mut column = vec![text("h2", "text-muted-foreground", None, title)];
                let number = text(
                    "h1",
                    &cn(&[display, "tabular-nums text-primary"]),
                    None,
                    count,
                );
                if full {
                    let mut readout = vec![number];
                    let range = get_string_or(entity, "rangeText", "");
                    if !range.is_empty() {
                        readout.push(text("small", "", Some("muted"), range));
                    }
                    column.push(pattern_spec(
                        "vstack",
                        [
                            ("gap", s("sm")),
                            ("align", s("center")),
                            ("children", array_value(readout)),
                        ],
                    ));
                } else {
                    column.push(number);
                }
                let (dec_label, inc_label) = if full {
                    (Some("decrementLabel"), Some("incrementLabel"))
                } else {
                    (None, None)
                };
                column.push(pattern_spec(
                    "hstack",
                    [
                        ("gap", s("md")),
                        (
                            "children",
                            array_value(vec![
                                button(
                                    entity,
                                    props,
                                    "secondary",
                                    button_size,
                                    "decrementEvent",
                                    "decrementDisabled",
                                    dec_label,
                                    "minus",
                                    None,
                                ),
                                button(
                                    entity,
                                    props,
                                    "primary",
                                    button_size,
                                    "incrementEvent",
                                    "incrementDisabled",
                                    inc_label,
                                    "plus",
                                    None,
                                ),
                            ]),
                        ),
                    ],
                ));
                if show_reset {
                    let reset_label = if full { "Reset to 0" } else { "Reset" };
                    column.push(button(
                        entity,
                        props,
                        "ghost",
                        "sm",
                        "resetEvent",
                        "",
                        None,
                        "rotate-ccw",
                        Some(reset_label),
                    ));
                }
                root.push(("size".to_string(), s("sm")));
                root.push(("padding".to_string(), s("lg")));
                if !class_name.is_empty() {
                    root.push(("className".to_string(), Value::string(class_name)));
                }
                root.push((
                    "children".to_string(),
                    array_value(vec![pattern_spec(
                        "vstack",
                        [
                            ("gap", s(if full { "xl" } else { "lg" })),
                            ("align", s("center")),
                            ("children", array_value(column)),
                        ],
                    )]),
                ));
                pattern_spec("container", root)
            }
        };
        registry.render_spec(&spec, None, view)
    }
}
