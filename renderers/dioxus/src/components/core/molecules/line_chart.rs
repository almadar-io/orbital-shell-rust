//! Native Dioxus body for the `line-chart` pattern, ported from web `LineChart.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct LineChartRenderer;

impl LineChartRenderer {
    const NAME: &str = "line-chart";
}

struct Datum {
    date: Option<f64>,
    x: Option<f64>,
    value: f64,
    label: Option<String>,
}

struct Plotted {
    x: f64,
    y: f64,
    value: f64,
    label: Option<String>,
}

fn num(value: f64) -> String {
    format!("{value}")
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_millis(value: &Value) -> f64 {
    match value {
        Value::Int(n) => *n as f64,
        Value::Number(n) => *n,
        Value::String(text) => {
            let text = text.to_string();
            let Some(date) = text.get(..10) else {
                return 0.0;
            };
            let mut parts = date.split('-');
            let (Some(y), Some(m), Some(d)) = (
                parts.next().and_then(|p| p.parse::<i64>().ok()),
                parts.next().and_then(|p| p.parse::<i64>().ok()),
                parts.next().and_then(|p| p.parse::<i64>().ok()),
            ) else {
                return 0.0;
            };
            let mut seconds = 0.0;
            if let Some(time) = text.get(11..) {
                let mut hms = time
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|p| !p.is_empty());
                for factor in [3600.0, 60.0, 1.0] {
                    if let Some(part) = hms.next().and_then(|p| p.parse::<f64>().ok()) {
                        seconds += part * factor;
                    }
                }
            }
            (days_from_civil(y, m, d) as f64) * 86_400_000.0 + seconds * 1000.0
        }
        _ => 0.0,
    }
}

fn gradient_id(values: &[f64], area_color: &str, line_color: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    for value in values {
        feed(&value.to_bits().to_le_bytes());
    }
    feed(area_color.as_bytes());
    feed(line_color.as_bytes());
    format!("line-chart-gradient-{hash:x}")
}

impl DioxusPatternRenderer for LineChartRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let width = get_number_or(props, "width", 400.0);
        let height = get_number_or(props, "height", 200.0);
        let show_grid = get_bool_or(props, "showGrid", true);
        let show_values = get_bool_or(props, "showValues", false);
        let show_area = get_bool_or(props, "showArea", true);
        let line_color = get_string_or(props, "lineColor", "var(--color-primary)");
        let area_color = get_string_or(props, "areaColor", "var(--color-primary)");
        let class_name = get_string_or(props, "className", "");

        let mut data: Vec<Datum> = get_array(props, "data")
            .iter()
            .map(|row| Datum {
                date: row.get("date").filter(|v| !v.is_null()).map(date_millis),
                x: get_number(row, "x"),
                value: get_number_or(row, "value", 0.0),
                label: row.get("label").and_then(|v| match v {
                    Value::String(text) => Some(text.to_string()),
                    Value::Int(n) => Some(n.to_string()),
                    Value::Number(n) => Some(num(*n)),
                    _ => None,
                }),
            })
            .collect();

        if data.is_empty() {
            let class =
                format!("flex items-center justify-center text-muted-foreground {class_name}");
            let style = format!("width:{}px;height:{}px", num(width), num(height));
            return rsx! { div { class: "{class}", style: "{style}", {crate::i18n::t("empty.noData")} } };
        }

        let has_date = data.iter().any(|d| d.date.is_some());
        let has_x = data.iter().any(|d| d.x.is_some());
        if has_date {
            data.sort_by(|a, b| {
                a.date
                    .unwrap_or(0.0)
                    .partial_cmp(&b.date.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else if has_x {
            data.sort_by(|a, b| {
                a.x.unwrap_or(0.0)
                    .partial_cmp(&b.x.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let values: Vec<f64> = data.iter().map(|d| d.value).collect();
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = if max_val - min_val == 0.0 {
            1.0
        } else {
            max_val - min_val
        };
        let padding = 20.0;
        let chart_width = width - padding * 2.0;
        let chart_height = height - padding * 2.0;
        let has_numeric_x = !has_date && has_x;
        let x_values: Vec<f64> = if has_numeric_x {
            data.iter().map(|d| d.x.unwrap_or(0.0)).collect()
        } else {
            vec![]
        };
        let min_x = x_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_range = if has_numeric_x {
            let r = x_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - min_x;
            if r == 0.0 {
                1.0
            } else {
                r
            }
        } else {
            1.0
        };

        let points: Vec<Plotted> = if data.len() == 1 {
            vec![Plotted {
                x: width / 2.0,
                y: height / 2.0,
                value: data[0].value,
                label: data[0].label.clone(),
            }]
        } else {
            let last = data.len() as f64 - 1.0;
            data.iter()
                .enumerate()
                .map(|(index, point)| Plotted {
                    x: if has_numeric_x {
                        padding + (point.x.unwrap_or(0.0) - min_x) / x_range * chart_width
                    } else {
                        padding + (index as f64 / last) * chart_width
                    },
                    y: padding + chart_height - ((point.value - min_val) / range) * chart_height,
                    value: point.value,
                    label: point.label.clone(),
                })
                .collect()
        };

        let line_path = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{} {} {}",
                    if i == 0 { "M" } else { "L" },
                    num(p.x),
                    num(p.y)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let area_path = if show_area {
            let bottom = height - 20.0;
            let first = &points[0];
            let last = &points[points.len() - 1];
            format!(
                "{line_path} L {} {} L {} {} Z",
                num(last.x),
                num(bottom),
                num(first.x),
                num(bottom)
            )
        } else {
            String::new()
        };

        let gid = gradient_id(&values, &area_color, &line_color);
        let fill_ref = format!("url(#{gid})");
        let vb = format!("0 0 {} {}", num(width), num(height));
        let root_class = merge_class_name("", props);
        let lone = points.len() == 1;
        let lone_text = points.first().map(|p| match &p.label {
            Some(label) => format!("{label}: {}", num(p.value)),
            None => num(p.value),
        });
        let grid_x2 = num(width - 20.0);

        rsx! {
            div { class: "{root_class}",
                svg { view_box: "{vb}", width: "100%", height: "100%", preserve_aspect_ratio: "xMidYMid meet",
                    defs {
                        linearGradient { id: "{gid}", x1: "0", y1: "0", x2: "0", y2: "1",
                            stop { offset: "0%", stop_color: "{area_color}", stop_opacity: "0.3" }
                            stop { offset: "100%", stop_color: "{area_color}", stop_opacity: "0" }
                        }
                    }
                    if show_grid {
                        for frac in [0.25_f64, 0.5, 0.75] {
                            line {
                                x1: "20",
                                y1: num(height * frac),
                                x2: "{grid_x2}",
                                y2: num(height * frac),
                                stroke: "var(--color-border, #e5e7eb)",
                                stroke_width: "1",
                            }
                        }
                    }
                    if show_area && !area_path.is_empty() {
                        path { d: "{area_path}", fill: "{fill_ref}" }
                    }
                    if !line_path.is_empty() {
                        path {
                            d: "{line_path}",
                            fill: "none",
                            stroke: "{line_color}",
                            stroke_width: "2",
                            stroke_linejoin: "round",
                            stroke_linecap: "round",
                        }
                    }
                    for point in points.iter() {
                        circle {
                            cx: num(point.x),
                            cy: num(point.y),
                            r: "4",
                            fill: "{line_color}",
                            stroke: "var(--color-background, white)",
                            stroke_width: "2",
                        }
                    }
                    if lone {
                        if let Some(content) = lone_text {
                            text {
                                x: num(points[0].x),
                                y: num(points[0].y - 12.0),
                                text_anchor: "middle",
                                font_size: "12",
                                fill: "var(--color-foreground, currentColor)",
                                "{content}"
                            }
                        }
                    }
                    if show_values && !lone {
                        for point in points.iter() {
                            {
                                let shown = point.label.clone().unwrap_or_else(|| num(point.value));
                                rsx! {
                            text {
                                x: num(point.x),
                                y: num(point.y - 10.0),
                                text_anchor: "middle",
                                font_size: "11",
                                fill: "var(--color-foreground, currentColor)",
                                "{shown}"
                            }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
