use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, layout_box_spec, object_value, pattern_spec as pattern,
    typography_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `meter` pattern (web: Meter.tsx).
pub struct MeterRenderer;

impl MeterRenderer {
    const NAME: &str = "meter";
}

struct Threshold {
    value: f64,
    color: String,
    label: Option<String>,
}

fn thresholds_of(props: &Value) -> Vec<Threshold> {
    match props.get("thresholds").and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(|item| {
                Some(Threshold {
                    value: get_number(item, "value")?,
                    color: get_string(item, "color")?,
                    label: get_string(item, "label").filter(|label| !label.is_empty()),
                })
            })
            .collect(),
        None => [
            (30.0, "var(--color-error)"),
            (70.0, "var(--color-warning)"),
            (100.0, "var(--color-success)"),
        ]
        .into_iter()
        .map(|(value, color)| Threshold {
            value,
            color: color.to_string(),
            label: None,
        })
        .collect(),
    }
}

fn color_for(value: f64, max: f64, thresholds: &[Threshold]) -> String {
    let percentage = (value / max) * 100.0;
    thresholds
        .iter()
        .find(|threshold| percentage <= threshold.value)
        .or(thresholds.last())
        .map(|threshold| threshold.color.clone())
        .unwrap_or_else(|| "var(--color-primary)".to_string())
}

fn number_text(value: f64) -> String {
    value.to_string()
}

fn action_badges(actions: &[Value], value: f64) -> Option<Value> {
    if actions.is_empty() {
        return None;
    }
    let badges = actions
        .iter()
        .map(|action| {
            let badge = pattern(
                "badge",
                [
                    ("variant", Value::string("default")),
                    (
                        "className",
                        Value::string("cursor-pointer hover:opacity-80 transition-opacity"),
                    ),
                    ("label", Value::string(get_string_or(action, "label", ""))),
                ],
            );
            let mut extra = Vec::new();
            if let Some(event) = get_string(action, "event") {
                extra.push(("action".to_string(), Value::string(event)));
                extra.push((
                    "actionPayload".to_string(),
                    object_value([("value", Value::Number(value))]),
                ));
            }
            extra.push(("display".to_string(), Value::string("inline-flex")));
            layout_box_spec("", None, extra, vec![badge])
        })
        .collect();
    Some(pattern(
        "hstack",
        [
            ("gap", Value::string("xs")),
            ("children", array_value(badges)),
        ],
    ))
}

impl DioxusPatternRenderer for MeterRenderer {
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
        let class_name = get_string_or(props, "className", "");
        if get_bool_or(props, "isLoading", false) {
            let mut fields = vec![("message", Value::string("Loading..."))];
            if !class_name.is_empty() {
                fields.push(("className", Value::string(class_name)));
            }
            return registry.render_spec(&pattern("loading-state", fields), None, view);
        }
        if let Some(message) = props
            .get("error")
            .and_then(|error| get_string(error, "message"))
        {
            let mut fields = vec![
                ("title", Value::string("Meter error")),
                ("message", Value::string(message)),
            ];
            if !class_name.is_empty() {
                fields.push(("className", Value::string(class_name)));
            }
            return registry.render_spec(&pattern("error-state", fields), None, view);
        }

        let min = get_number_or(props, "min", 0.0);
        let max = get_number_or(props, "max", 100.0);
        let safe_val = get_number_or(props, "value", 0.0);
        let label = get_string(props, "label").filter(|label| !label.is_empty());
        let unit = get_string(props, "unit").filter(|unit| !unit.is_empty());
        let variant = get_string_or(props, "variant", "linear");
        let show_value = get_bool_or(props, "showValue", true);
        let thresholds = thresholds_of(props);
        let range = max - min;
        let percentage = if range <= 0.0 {
            0.0
        } else {
            (((safe_val - min) / range) * 100.0).clamp(0.0, 100.0)
        };
        let active_color = color_for(safe_val, max, &thresholds);
        let formatted = if safe_val.fract() == 0.0 {
            number_text(safe_val)
        } else {
            format!("{safe_val:.1}")
        };
        let display_value = match &unit {
            Some(unit) => format!("{formatted}{unit}"),
            None => formatted,
        };
        let aria_label = get_string(props, "aria-label").or_else(|| label.clone());
        let mut meter_attrs = vec![("role".to_string(), Value::string("meter"))];
        if let Some(aria_label) = aria_label {
            meter_attrs.push(("aria-label".to_string(), Value::string(aria_label)));
        }
        let actions = get_array(props, "actions");
        let card_class = cn(&["p-4", &class_name]);
        let passthrough = dom_passthrough(props);
        let card = |children: Vec<Value>| {
            let mut fields = passthrough.clone();
            fields.push(("className".to_string(), Value::string(card_class.clone())));
            fields.push(("children".to_string(), array_value(children)));
            pattern("card", fields)
        };
        let label_text =
            |label: &str| typography_spec("small", Some("secondary"), "font-medium", label);
        let value_text =
            |text: &str| typography_spec("small", None, "font-bold tabular-nums", text);
        let header = || -> Option<Value> {
            if label.is_none() && !show_value {
                return None;
            }
            let mut row = Vec::new();
            if let Some(label) = &label {
                row.push(label_text(label));
            }
            if show_value {
                row.push(value_text(&display_value));
            }
            Some(pattern(
                "hstack",
                [
                    ("justify", Value::string("between")),
                    ("align", Value::string("center")),
                    ("children", array_value(row)),
                ],
            ))
        };

        if variant == "radial" {
            let (size, stroke, font_size) = match get_string_or(props, "size", "md").as_str() {
                "sm" => (80.0_f64, 6.0_f64, "12px"),
                "lg" => (160.0, 10.0, "20px"),
                _ => (120.0, 8.0, "16px"),
            };
            let radius = (size - stroke * 2.0) / 2.0;
            let circumference = 2.0 * std::f64::consts::PI * radius;
            let offset = circumference - (percentage / 100.0) * circumference;
            let center = size / 2.0;
            let label_spec = label.as_ref().map(|label| label_text(label));
            let overlay = show_value.then(|| {
                layout_box_spec(
                    "absolute inset-0 flex items-center justify-center",
                    None,
                    Vec::new(),
                    vec![pattern(
                        "typography",
                        [
                            ("variant", Value::string("h5")),
                            ("className", Value::string("tabular-nums")),
                            ("content", Value::string(display_value.clone())),
                            (
                                "style",
                                object_value([("fontSize", Value::string(font_size))]),
                            ),
                        ],
                    )],
                )
            });
            let actions_spec = action_badges(&actions, safe_val);
            let root_class = format!("rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md {card_class}");
            let aria = get_string(props, "aria-label").or_else(|| label.clone());
            let dasharray = circumference.to_string();
            let dashoffset = offset.to_string();
            let stroke_text = stroke.to_string();
            let radius_text = radius.to_string();
            let center_text = center.to_string();
            let size_text = size.to_string();
            return rsx! {
                div { class: "{root_class}", style: "height:auto",
                    div { class: "flex flex-col gap-2 items-center justify-start", style: "height:auto",
                        if let Some(spec) = label_spec { {registry.render_spec(&spec, None, view)} }
                        div { class: "relative inline-flex items-center justify-center", role: "meter", aria_label: aria,
                            svg { width: "{size_text}", height: "{size_text}", view_box: "0 0 {size_text} {size_text}", class: "transform -rotate-90",
                                circle { cx: "{center_text}", cy: "{center_text}", r: "{radius_text}", fill: "none", stroke: "var(--color-muted)", stroke_width: "{stroke_text}" }
                                circle { cx: "{center_text}", cy: "{center_text}", r: "{radius_text}", fill: "none", stroke: "{active_color}", stroke_width: "{stroke_text}", stroke_dasharray: "{dasharray}", stroke_dashoffset: "{dashoffset}", stroke_linecap: "round", class: "transition-all duration-slow ease-emphasized" }
                            }
                            if let Some(spec) = overlay { {registry.render_spec(&spec, None, view)} }
                        }
                        if let Some(spec) = actions_spec { {registry.render_spec(&spec, None, view)} }
                    }
                }
            };
        }

        let mut column = Vec::new();
        if variant == "segmented" {
            let segments = get_number_or(props, "segments", 5.0).max(0.0) as usize;
            let active_segments = ((percentage / 100.0) * segments as f64).round() as usize;
            column.extend(header());
            let bars = (0..segments)
                .map(|index| {
                    let active = index < active_segments;
                    let class = cn(&[
                        "h-2 flex-1 rounded-sm transition-all duration-normal",
                        if active { "" } else { "bg-muted" },
                    ]);
                    let style = active.then(|| {
                        let color = color_for(
                            ((index + 1) as f64 / segments as f64) * max,
                            max,
                            &thresholds,
                        );
                        object_value([("backgroundColor", Value::string(color))])
                    });
                    layout_box_spec(&class, style, Vec::new(), Vec::new())
                })
                .collect();
            let mut bar_fields = meter_attrs.clone();
            bar_fields.push(("gap".to_string(), Value::string("xs")));
            bar_fields.push(("className".to_string(), Value::string("w-full")));
            bar_fields.push(("children".to_string(), array_value(bars)));
            column.push(pattern("hstack", bar_fields));
            if thresholds.iter().any(|threshold| threshold.label.is_some()) {
                let labels = thresholds
                    .iter()
                    .map(|threshold| {
                        typography_spec(
                            "caption",
                            Some("secondary"),
                            "",
                            threshold.label.as_deref().unwrap_or(""),
                        )
                    })
                    .collect();
                column.push(pattern(
                    "hstack",
                    [
                        ("justify", Value::string("between")),
                        ("className", Value::string("w-full")),
                        ("children", array_value(labels)),
                    ],
                ));
            }
        } else {
            column.extend(header());
            let unit_suffix = unit
                .as_ref()
                .map(|unit| format!(" {unit}"))
                .unwrap_or_default();
            column.push(layout_box_spec(
                "w-full h-3 bg-muted rounded-full overflow-hidden",
                None,
                meter_attrs.clone(),
                vec![layout_box_spec(
                    "h-full rounded-full transition-all duration-slow ease-emphasized",
                    Some(object_value([
                        ("width", Value::string(format!("{percentage}%"))),
                        ("backgroundColor", Value::string(active_color)),
                    ])),
                    Vec::new(),
                    Vec::new(),
                )],
            ));
            column.push(pattern(
                "hstack",
                [
                    ("justify", Value::string("between")),
                    ("className", Value::string("w-full")),
                    (
                        "children",
                        array_value(vec![
                            typography_spec(
                                "caption",
                                Some("secondary"),
                                "",
                                &format!("{}{unit_suffix}", number_text(min)),
                            ),
                            typography_spec(
                                "caption",
                                Some("secondary"),
                                "",
                                &format!("{}{unit_suffix}", number_text(max)),
                            ),
                        ]),
                    ),
                ],
            ));
            column.extend(action_badges(&actions, safe_val));
        }
        let stack = pattern(
            "vstack",
            [
                ("gap", Value::string("sm")),
                ("children", array_value(column)),
            ],
        );
        registry.render_spec(&card(vec![stack]), None, view)
    }
}
