use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, format_display_value, format_grouped_number, object_value,
    pattern_spec as pattern, plain_text, typography_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `stat-display` pattern (web: StatDisplay.tsx).
pub struct StatDisplayRenderer;

impl StatDisplayRenderer {
    const NAME: &str = "stat-display";
}

fn variant_color(variant: &str) -> &'static str {
    match variant {
        "primary" => "text-primary",
        "success" => "text-success",
        "warning" => "text-warning",
        "error" => "text-error",
        "info" => "text-info",
        _ => "text-foreground",
    }
}

fn number_of(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Int(whole) => Some(*whole as f64),
        Value::Number(float) => Some(*float),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn icon_spec(name: &str, class: &str) -> Value {
    pattern(
        "icon",
        [
            ("name", Value::string(name)),
            ("className", Value::string(class)),
        ],
    )
}

impl DioxusPatternRenderer for StatDisplayRenderer {
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
        let size = get_string_or(props, "size", "md");
        let variant = get_string_or(props, "variant", "default");
        let surface_class = if get_string_or(props, "surface", "auto") == "auto" {
            "surface-content"
        } else {
            ""
        };
        let icon_size = match size.as_str() {
            "sm" => "h-4 w-4",
            "lg" => "h-6 w-6",
            _ => "h-5 w-5",
        };
        let value_size = match size.as_str() {
            "sm" => "text-lg",
            "lg" => "text-3xl",
            _ => "text-2xl",
        };
        let pad = match size.as_str() {
            "sm" => "p-3",
            "lg" => "p-6",
            _ => "p-4",
        };
        let passthrough = dom_passthrough(props);
        let with_passthrough = |mut fields: Vec<(String, Value)>| {
            fields.extend(passthrough.iter().cloned());
            fields
        };

        if let Some(message) = props
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str().map(str::to_string))
        {
            let fields = with_passthrough(vec![
                (
                    "className".to_string(),
                    Value::string(cn(&[surface_class, "rounded-container", pad, &class_name])),
                ),
                (
                    "children".to_string(),
                    array_value(vec![typography_spec("small", Some("error"), "", &message)]),
                ),
            ]);
            return registry.render_spec(&pattern("box", fields), None, view);
        }
        if get_bool_or(props, "isLoading", false) {
            let spec = props.get("skeleton").cloned().unwrap_or_else(|| {
                object_value([
                    ("variant", Value::string("stats")),
                    ("columns", Value::Int(1)),
                ])
            });
            let mut fields = vec![("spec", spec)];
            if !class_name.is_empty() {
                fields.push(("className", Value::string(class_name.clone())));
            }
            return registry.render_spec(&pattern("skeleton", fields), None, view);
        }

        let label = props
            .get("label")
            .map(|v| plain_text(&v))
            .unwrap_or_default();
        let value = props.get("value");
        let format = get_string(props, "format");
        let formatted = match value {
            Some(value) if !value.is_null() => format_display_value(&value, format.as_deref()),
            _ => "0".to_string(),
        };
        let max = get_number_or(props, "max", 0.0);
        let with_max = if props.get("max").is_some() && max > 0.0 {
            format!("{formatted} / {}", format_grouped_number(max, 0, 3))
        } else {
            formatted
        };
        let display_value = format!(
            "{}{with_max}{}",
            get_string_or(props, "prefix", ""),
            get_string_or(props, "suffix", "")
        );
        let numeric = number_of(value);
        let target = get_number(props, "target").filter(|target| *target > 0.0);
        let show_target = target.is_some() && numeric.is_some_and(f64::is_finite);
        let target_pct = match (target, numeric) {
            (Some(target), Some(numeric)) => ((numeric / target) * 100.0).clamp(0.0, 100.0),
            _ => 0.0,
        };
        let trend = get_number(props, "trend").filter(|trend| *trend != 0.0 && trend.is_finite());
        let trend_up = trend.is_some_and(|trend| trend > 0.0);
        let trend_is_good =
            if get_string_or(props, "trendPolarity", "higher-is-better") == "lower-is-better" {
                !trend_up
            } else {
                trend_up
            };
        let trend_label = trend.map(|trend| {
            let suffix = if get_string_or(props, "trendFormat", "absolute") == "percent" {
                "%"
            } else {
                ""
            };
            format!(
                "{} {}{suffix}",
                if trend_up { "↑" } else { "↓" },
                trend.abs()
            )
        });
        let trend_color = if trend_is_good {
            "text-success"
        } else {
            "text-error"
        };
        let sparkline = props
            .get("sparklineData")
            .filter(|data| data.as_array().is_some_and(|items| items.len() > 1));
        let icon_name = get_string(props, "icon");
        let icon_bg = get_string_or(props, "iconBg", "bg-muted");
        let icon_color = get_string_or(props, "iconColor", "text-foreground");
        let click_event = get_string(props, "clickEvent");
        let click_fields = |mut fields: Vec<(String, Value)>| {
            if let Some(event) = &click_event {
                fields.push(("action".to_string(), Value::string(event.clone())));
                fields.push((
                    "actionPayload".to_string(),
                    object_value([("metricLabel", Value::string(label.clone()))]),
                ));
                fields.push(("role".to_string(), Value::string("button")));
                fields.push(("tabIndex".to_string(), Value::Int(0)));
            }
            fields
        };
        let value_text = typography_spec(
            "h4",
            None,
            &cn(&[value_size, variant_color(&variant)]),
            &display_value,
        );

        if get_bool_or(props, "compact", false) {
            let mut children = Vec::new();
            if let Some(name) = &icon_name {
                children.push(icon_spec(name, &cn(&[icon_size, &icon_color])));
            }
            children.push(typography_spec("caption", Some("secondary"), "", &label));
            children.push(value_text);
            if let Some(trend_label) = &trend_label {
                children.push(typography_spec(
                    "caption",
                    None,
                    &cn(&["font-semibold", trend_color]),
                    trend_label,
                ));
            }
            if let Some(data) = sparkline {
                children.push(pattern(
                    "sparkline",
                    [
                        ("data", data.clone()),
                        ("color", Value::string("auto")),
                        ("width", Value::Int(60)),
                        ("height", Value::Int(20)),
                    ],
                ));
            }
            let fields = with_passthrough(click_fields(vec![
                ("gap".to_string(), Value::string("sm")),
                (
                    "className".to_string(),
                    Value::string(cn(&[
                        "items-center",
                        if click_event.is_some() {
                            "cursor-pointer hover:opacity-80"
                        } else {
                            ""
                        },
                        &class_name,
                    ])),
                ),
                ("children".to_string(), array_value(children)),
            ]));
            return registry.render_spec(&pattern("hstack", fields), None, view);
        }

        let mut label_column = vec![
            typography_spec("overline", Some("secondary"), "", &label),
            pattern(
                "hstack",
                [
                    ("gap", Value::string("sm")),
                    ("align", Value::string("end")),
                    (
                        "children",
                        array_value({
                            let mut row = vec![value_text];
                            if let Some(trend_label) = &trend_label {
                                row.push(typography_spec(
                                    "caption",
                                    None,
                                    &cn(&["font-semibold pb-1", trend_color]),
                                    trend_label,
                                ));
                            }
                            row
                        }),
                    ),
                ],
            ),
        ];
        if show_target {
            let fill = if variant == "default" {
                "primary".to_string()
            } else {
                variant.clone()
            };
            label_column.push(pattern(
                "box",
                [
                    (
                        "className",
                        Value::string("mt-2 h-1.5 w-full bg-muted rounded-full overflow-hidden"),
                    ),
                    (
                        "children",
                        array_value(vec![pattern(
                            "box",
                            [
                                (
                                    "className",
                                    Value::string(format!(
                                        "h-full rounded-full transition-all bg-{fill}"
                                    )),
                                ),
                                (
                                    "style",
                                    object_value([(
                                        "width",
                                        Value::string(format!("{target_pct}%")),
                                    )]),
                                ),
                            ],
                        )]),
                    ),
                ],
            ));
        }
        let mut side = Vec::new();
        if let Some(name) = &icon_name {
            side.push(pattern(
                "box",
                [
                    (
                        "className",
                        Value::string(cn(&["p-3 rounded-container", &icon_bg])),
                    ),
                    (
                        "children",
                        array_value(vec![icon_spec(name, &cn(&[icon_size, &icon_color]))]),
                    ),
                ],
            ));
        }
        if let Some(data) = sparkline {
            side.push(pattern(
                "sparkline",
                [("data", data.clone()), ("color", Value::string("auto"))],
            ));
        }
        let layout = pattern(
            "hstack",
            [
                ("align", Value::string("start")),
                ("justify", Value::string("between")),
                (
                    "children",
                    array_value(vec![
                        pattern(
                            "vstack",
                            [
                                ("gap", Value::string("none")),
                                ("className", Value::string("space-y-1 flex-1")),
                                ("children", array_value(label_column)),
                            ],
                        ),
                        pattern(
                            "vstack",
                            [
                                ("gap", Value::string("xs")),
                                ("align", Value::string("end")),
                                ("children", array_value(side)),
                            ],
                        ),
                    ]),
                ),
            ],
        );
        let look = if get_string_or(props, "look", "elevated") == "flat" {
            "shadow-none"
        } else {
            ""
        };
        let fields = with_passthrough(click_fields(vec![
            (
                "className".to_string(),
                Value::string(cn(&[
                    surface_class,
                    "rounded-container",
                    pad,
                    look,
                    if click_event.is_some() {
                        "cursor-pointer hover:shadow-elevation-dialog transition-shadow"
                    } else {
                        ""
                    },
                    &class_name,
                ])),
            ),
            ("children".to_string(), array_value(vec![layout])),
        ]));
        registry.render_spec(&pattern("box", fields), None, view)
    }
}
