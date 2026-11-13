use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, format_display_value, format_grouped_number, object_value,
    pattern_spec as pattern, plain_text, typography_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `stat-card` pattern (web: StatCard.tsx).
pub struct StatCardRenderer;

impl StatCardRenderer {
    const NAME: &str = "stat-card";
}

fn number_of(value: &Value) -> Option<f64> {
    match value {
        Value::Int(whole) => Some(*whole as f64),
        Value::Number(float) => Some(*float),
        _ => None,
    }
}

fn skeleton_bar(class: &str) -> Value {
    pattern("box", [("className", Value::string(class))])
}

fn with_props(props: &Value, mut fields: Vec<(String, Value)>) -> Vec<(String, Value)> {
    fields.extend(dom_passthrough(props));
    fields
}

impl DioxusPatternRenderer for StatCardRenderer {
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
        let surface_class = if get_string_or(props, "surface", "auto") == "auto" {
            "surface-content"
        } else {
            ""
        };
        let tile_class = cn(&[
            if surface_class.is_empty() {
                "rounded-container border border-border"
            } else {
                surface_class
            },
            "p-4",
        ]);
        let grid_class = cn(&[
            "grid gap-4 grid-cols-[repeat(auto-fit,minmax(min(10rem,100%),1fr))]",
            &class_name,
        ]);
        let metrics = get_array(props, "metrics");
        let loading = get_bool_or(props, "isLoading", false);
        let error_message = props
            .get("error")
            .and_then(|error| error.get("message"))
            .map(plain_text);
        let box_spec = |class: String, children: Vec<Value>| {
            pattern(
                "box",
                with_props(
                    props,
                    vec![
                        ("className".to_string(), Value::string(class)),
                        ("children".to_string(), array_value(children)),
                    ],
                ),
            )
        };

        if metrics.len() > 1 {
            let tiles: Vec<Value> = if loading {
                metrics
                    .iter()
                    .map(|_| {
                        pattern(
                            "box",
                            [
                                ("className", Value::string(tile_class.clone())),
                                (
                                    "children",
                                    array_value(vec![pattern(
                                        "vstack",
                                        [
                                            ("gap", Value::string("xs")),
                                            ("className", Value::string("animate-pulse")),
                                            (
                                                "children",
                                                array_value(vec![
                                                    skeleton_bar("h-3 bg-muted rounded w-16"),
                                                    skeleton_bar("h-6 bg-muted rounded w-12"),
                                                ]),
                                            ),
                                        ],
                                    )]),
                                ),
                            ],
                        )
                    })
                    .collect()
            } else {
                metrics
                    .iter()
                    .map(|metric| {
                        let format = get_string(metric, "format");
                        let value = metric
                            .get("value")
                            .map(|value| {
                                format_display_value(
                                    value,
                                    Some(format.as_deref().unwrap_or("number")),
                                )
                            })
                            .unwrap_or_default();
                        pattern(
                            "box",
                            [
                                ("className", Value::string(tile_class.clone())),
                                (
                                    "children",
                                    array_value(vec![
                                        typography_spec(
                                            "overline",
                                            Some("secondary"),
                                            "",
                                            &get_string_or(metric, "label", ""),
                                        ),
                                        typography_spec("h4", None, "text-xl tabular-nums", &value),
                                    ]),
                                ),
                            ],
                        )
                    })
                    .collect()
            };
            return registry.render_spec(&box_spec(grid_class, tiles), None, view);
        }

        let first = metrics.first();
        let label = first
            .and_then(|metric| get_string(metric, "label"))
            .filter(|label| !label.is_empty())
            .or_else(|| get_string(props, "label").or_else(|| get_string(props, "title")))
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| crate::i18n::t("statCard.defaultLabel"));
        let panel_class = cn(&[surface_class, "p-6 rounded-container", &class_name]);

        if let Some(message) = error_message {
            return registry.render_spec(
                &box_spec(
                    panel_class,
                    vec![pattern(
                        "vstack",
                        [
                            ("gap", Value::string("none")),
                            ("className", Value::string("space-y-1")),
                            (
                                "children",
                                array_value(vec![
                                    typography_spec("overline", Some("secondary"), "", &label),
                                    typography_spec(
                                        "small",
                                        Some("error"),
                                        "",
                                        &format!("Something went wrong: {message}"),
                                    ),
                                ]),
                            ),
                        ],
                    )],
                ),
                None,
                view,
            );
        }
        if loading {
            return registry.render_spec(
                &box_spec(
                    panel_class,
                    vec![pattern(
                        "vstack",
                        [
                            ("gap", Value::string("sm")),
                            ("className", Value::string("animate-pulse")),
                            (
                                "children",
                                array_value(vec![
                                    skeleton_bar("h-4 bg-muted rounded w-24"),
                                    skeleton_bar("h-8 bg-muted rounded w-32"),
                                    skeleton_bar("h-4 bg-muted rounded w-20"),
                                ]),
                            ),
                        ],
                    )],
                ),
                None,
                view,
            );
        }

        let manual_trend = get_number(props, "trend");
        let calculated_trend = manual_trend.or_else(|| {
            let previous = get_number(props, "previousValue")?;
            let current = get_number(props, "currentValue")?;
            Some(if previous == 0.0 {
                if current > 0.0 {
                    100.0
                } else {
                    0.0
                }
            } else {
                ((current - previous) / previous) * 100.0
            })
        });
        let direction = get_string(props, "trendDirection").unwrap_or_else(|| {
            match calculated_trend {
                Some(trend) if trend > 0.0 => "up",
                Some(trend) if trend < 0.0 => "down",
                _ => "neutral",
            }
            .to_string()
        });
        let is_positive = if get_bool_or(props, "invertTrend", false) {
            direction == "down"
        } else {
            direction == "up"
        };
        let trend_icon = match direction.as_str() {
            "up" => "trending-up",
            "down" => "trending-down",
            _ => "minus",
        };
        let trend_color = if is_positive {
            "text-success"
        } else if direction == "neutral" {
            "text-muted-foreground"
        } else {
            "text-error"
        };

        let prop_value = props.get("value").map(|value| match value {
            Value::Array(items) => items
                .first()
                .filter(|item| !item.is_null())
                .cloned()
                .unwrap_or(Value::Int(items.len() as i64)),
            other => other.clone(),
        });
        let value = first
            .and_then(|metric| metric.get("value").cloned())
            .or(prop_value)
            .unwrap_or(Value::Int(0));
        let value_format = first
            .and_then(|metric| get_string(metric, "format"))
            .unwrap_or_else(|| "number".to_string());
        let value_text = match number_of(&value) {
            Some(_) => format_display_value(&value, Some(&value_format)),
            None => plain_text(&value),
        };

        let mut column = vec![
            typography_spec("overline", Some("secondary"), "", &label),
            typography_spec("h4", None, "text-2xl tabular-nums", &value_text),
        ];
        if let Some(trend) = calculated_trend {
            let percent = format!("{}%", format_grouped_number(trend.abs(), 0, 1));
            column.push(pattern(
                "hstack",
                [
                    ("align", Value::string("center")),
                    ("gap", Value::string("xs")),
                    (
                        "children",
                        array_value(vec![
                            pattern(
                                "hstack",
                                [
                                    ("align", Value::string("center")),
                                    ("gap", Value::string("none")),
                                    (
                                        "className",
                                        Value::string(cn(&[
                                            "gap-0.5 text-sm font-bold",
                                            trend_color,
                                        ])),
                                    ),
                                    (
                                        "children",
                                        array_value(vec![
                                            pattern(
                                                "icon",
                                                [
                                                    ("name", Value::string(trend_icon)),
                                                    ("className", Value::string("h-4 w-4")),
                                                ],
                                            ),
                                            typography_spec("caption", None, "", &percent),
                                        ]),
                                    ),
                                ],
                            ),
                            typography_spec(
                                "small",
                                Some("secondary"),
                                "",
                                &crate::i18n::t("statCard.vsLastPeriod"),
                            ),
                        ]),
                    ),
                ],
            ));
        }
        if let Some(subtitle) = get_string(props, "subtitle").filter(|text| !text.is_empty()) {
            if calculated_trend.is_none_or(|trend| trend == 0.0) {
                column.push(typography_spec("small", Some("secondary"), "", &subtitle));
            }
        }

        let mut side = Vec::new();
        if let Some(name) = get_string(props, "icon") {
            side.push(pattern(
                "box",
                [
                    (
                        "className",
                        Value::string(cn(&["p-3", &get_string_or(props, "iconBg", "bg-muted")])),
                    ),
                    (
                        "children",
                        array_value(vec![pattern(
                            "icon",
                            [
                                ("name", Value::string(name)),
                                (
                                    "className",
                                    Value::string(cn(&[
                                        "h-6 w-6",
                                        &get_string_or(props, "iconColor", "text-foreground"),
                                    ])),
                                ),
                            ],
                        )]),
                    ),
                ],
            ));
        }
        if let Some(data) = props
            .get("sparklineData")
            .filter(|data| data.as_array().is_some_and(|items| items.len() > 1))
        {
            side.push(pattern(
                "sparkline",
                [("data", data.clone()), ("color", Value::string("auto"))],
            ));
        }

        let mut children = vec![pattern(
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
                                ("className", Value::string("space-y-1")),
                                ("children", array_value(column)),
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
        )];
        if let Some(action) = props.get("action") {
            let mut fields = vec![
                ("variant", Value::string("ghost")),
                (
                    "className",
                    Value::string("mt-4 text-sm font-bold text-foreground hover:underline"),
                ),
                (
                    "label",
                    Value::string(format!("{} →", get_string_or(action, "label", ""))),
                ),
            ];
            if let Some(event) = get_string(action, "event") {
                fields.push(("action", Value::string(event)));
                fields.push(("actionPayload", object_value::<&str>([])));
            }
            children.push(pattern("button", fields));
        }
        registry.render_spec(&box_spec(panel_class, children), None, view)
    }
}
