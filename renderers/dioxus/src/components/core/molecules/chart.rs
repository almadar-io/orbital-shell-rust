//! Native Dioxus body for the `chart` pattern, ported from web `Chart.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct ChartRenderer;

impl ChartRenderer {
    const NAME: &str = "chart";
}

const CHART_COLORS: [&str; 6] = [
    "var(--color-primary)",
    "var(--color-success)",
    "var(--color-warning)",
    "var(--color-error)",
    "var(--color-info)",
    "var(--color-accent)",
];

const FOCUS_SVG: &str =
    "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2";

struct Point {
    label: String,
    value: f64,
    color: Option<String>,
}

struct Series {
    name: String,
    color: Option<String>,
    dashed: bool,
    data: Vec<Point>,
}

struct ScatterPoint {
    x: f64,
    y: f64,
    label: Option<String>,
    size: Option<f64>,
    color: Option<String>,
}

fn node(name: &str, mut fields: Vec<(&str, Value)>, children: Vec<Value>) -> Value {
    if !children.is_empty() {
        fields.push(("children", Value::Array(children.into())));
    }
    pattern_spec(name, fields)
}

fn css(pairs: &[(&str, String)]) -> Value {
    object_value(pairs.iter().map(|(k, v)| (*k, Value::string(v.clone()))))
}

fn typo(
    content: String,
    variant: &str,
    color: Option<&str>,
    class: Option<&str>,
    style: Option<Value>,
) -> Value {
    let mut fields = vec![
        ("content", Value::string(content)),
        ("variant", Value::string(variant)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if let Some(class) = class {
        fields.push(("className", Value::string(class)));
    }
    if let Some(style) = style {
        fields.push(("style", style));
    }
    node("typography", fields, vec![])
}

fn stack(
    name: &str,
    gap: &str,
    align: Option<&str>,
    class: Option<&str>,
    style: Option<Value>,
    children: Vec<Value>,
) -> Value {
    let mut fields = vec![("gap", Value::string(gap))];
    if let Some(align) = align {
        fields.push(("align", Value::string(align)));
    }
    if let Some(class) = class {
        fields.push(("className", Value::string(class)));
    }
    if let Some(style) = style {
        fields.push(("style", style));
    }
    node(name, fields, children)
}

fn num(value: f64) -> String {
    format!("{value}")
}

fn group_digits(int_part: &str) -> String {
    let digits: Vec<char> = int_part.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    out
}

fn fmt_fixed(value: f64, min_fraction: usize, max_fraction: usize) -> String {
    let negative = value < 0.0;
    let mut text = format!("{:.*}", max_fraction, value.abs());
    if let Some(dot) = text.find('.') {
        let mut end = text.len();
        while end > dot + 1 + min_fraction && text.as_bytes()[end - 1] == b'0' {
            end -= 1;
        }
        if end == dot + 1 {
            end = dot;
        }
        text.truncate(end);
    }
    let (int_part, rest) = match text.find('.') {
        Some(dot) => (text[..dot].to_string(), text[dot..].to_string()),
        None => (text.clone(), String::new()),
    };
    format!(
        "{}{}{}",
        if negative { "-" } else { "" },
        group_digits(&int_part),
        rest
    )
}

fn fmt_number(value: f64) -> String {
    fmt_fixed(value, 0, 3)
}

fn point_name(label: &str, series_name: &str, value: f64) -> String {
    if series_name == "default" {
        format!("{label}: {}", num(value))
    } else {
        format!("{label}, {series_name}: {}", num(value))
    }
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Int(n) => Some(n.to_string()),
        Value::Number(n) => Some(num(*n)),
        _ => None,
    }
}

fn field_text(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(text_of)
}

fn parse_points(rows: &[Value]) -> Vec<Point> {
    rows.iter()
        .map(|row| Point {
            label: field_text(row, "label").unwrap_or_default(),
            value: get_number_or(row, "value", 0.0),
            color: get_string(row, "color"),
        })
        .collect()
}

fn series_color(series: &Series, idx: usize) -> String {
    series
        .color
        .clone()
        .unwrap_or_else(|| CHART_COLORS[idx % CHART_COLORS.len()].to_string())
}

fn value_at(series: &Series, label: &str) -> f64 {
    series
        .data
        .iter()
        .find(|point| point.label == label)
        .map(|point| point.value)
        .unwrap_or(0.0)
}

fn categories(series: &[Series]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in series {
        for p in &s.data {
            if !out.contains(&p.label) {
                out.push(p.label.clone());
            }
        }
    }
    out
}

fn max_value(series: &[Series], stack_mode: &str, totals: &[f64]) -> f64 {
    if stack_mode == "normalize" {
        return 100.0;
    }
    if stack_mode == "stack" {
        return totals.iter().cloned().fold(1.0, f64::max);
    }
    let mut m = 1.0;
    for s in series {
        for p in &s.data {
            if p.value > m {
                m = p.value;
            }
        }
    }
    m
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse_utc_date(raw: &str, epoch_ms: bool) -> Option<(i64, u32, u32)> {
    if epoch_ms {
        let ms: i64 = raw.parse().ok()?;
        return Some(civil_from_days(ms.div_euclid(86_400_000)));
    }
    let date = raw.get(..10)?;
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || date.len() != 10 {
        return None;
    }
    if raw.len() > 10 && !raw[10..].starts_with('T') {
        return None;
    }
    Some((y, m, d))
}

fn format_time_label(raw: &str, period: Option<&str>) -> String {
    let epoch_ms = period.is_some()
        && !raw.is_empty()
        && raw
            .trim_start_matches('-')
            .chars()
            .all(|c| c.is_ascii_digit())
        && raw != "-";
    let Some((y, m, d)) = parse_utc_date(raw, epoch_ms) else {
        return raw.to_string();
    };
    let day = format!("{} {}", MONTHS[(m - 1) as usize], d);
    match period {
        Some("day") => day,
        Some("week") => format!("Week of {day}"),
        Some("quarter") => format!("Q{} {}", (m - 1) / 3 + 1, y),
        Some("year") => y.to_string(),
        _ => format!("{} {}", MONTHS[(m - 1) as usize], y),
    }
}

fn drill_props(
    drill: &Option<String>,
    label: &str,
    value: f64,
    series_name: &str,
) -> Vec<(&'static str, Value)> {
    let Some(event) = drill else {
        return vec![];
    };
    let mut payload = vec![
        ("label", Value::string(label)),
        ("value", Value::Number(value)),
    ];
    if series_name != "default" {
        payload.push(("seriesLabel", Value::string(series_name)));
    }
    vec![
        ("action", Value::string(event.clone())),
        ("actionPayload", object_value(payload)),
        ("role", Value::string("button")),
        (
            "aria-label",
            Value::string(point_name(label, series_name, value)),
        ),
    ]
}

fn bar_chart(
    series: &[Series],
    height: f64,
    show_values: bool,
    stack_mode: &str,
    time_axis: bool,
    period: Option<&str>,
    histogram: bool,
    horizontal: bool,
    drill: &Option<String>,
) -> Value {
    let cats = categories(series);
    let totals: Vec<f64> = if stack_mode == "none" {
        vec![]
    } else {
        cats.iter()
            .map(|label| series.iter().map(|s| value_at(s, label)).sum())
            .collect()
    };
    let max = max_value(series, stack_mode, &totals);
    let display = |label: &str| {
        if time_axis {
            format_time_label(label, period)
        } else {
            label.to_string()
        }
    };
    let ratio_of = |value: f64, cat_idx: usize| -> f64 {
        if stack_mode == "normalize" {
            let total = totals.get(cat_idx).copied().unwrap_or(1.0);
            if total == 0.0 {
                0.0
            } else {
                value / total * 100.0
            }
        } else {
            value / max * 100.0
        }
    };

    if horizontal {
        let rows = cats
            .iter()
            .enumerate()
            .map(|(cat_idx, label)| {
                let bars: Vec<Value> = series
                    .iter()
                    .enumerate()
                    .map(|(s_idx, s)| {
                        let value = value_at(s, label);
                        let color = series_color(s, s_idx);
                        let mut fields = vec![
                            (
                                "className",
                                Value::string("h-full rounded-r-sm transition-all duration-slow ease-standard min-w-[2px] cursor-pointer hover:opacity-80"),
                            ),
                            (
                                "style",
                                css(&[("width", format!("{}%", num(ratio_of(value, cat_idx)))), ("backgroundColor", color)]),
                            ),
                        ];
                        fields.extend(drill_props(drill, label, value, &s.name));
                        node("box", fields, vec![])
                    })
                    .collect();
                let mut cells = vec![
                    typo(
                        display(label),
                        "caption",
                        Some("secondary"),
                        Some("truncate text-right"),
                        Some(css(&[("width", "80px".into()), ("flexShrink", "0".into())])),
                    ),
                    stack(
                        "hstack",
                        if stack_mode == "none" { "xs" } else { "none" },
                        Some("center"),
                        Some("flex-1 min-w-0"),
                        Some(css(&[("height", "24px".into())])),
                        bars,
                    ),
                ];
                if show_values && series.len() == 1 {
                    cells.push(typo(
                        num(value_at(&series[0], label)),
                        "caption",
                        Some("secondary"),
                        Some("tabular-nums"),
                        Some(css(&[("width", "40px".into()), ("flexShrink", "0".into())])),
                    ));
                }
                stack("hstack", "sm", Some("center"), Some("w-full"), None, cells)
            })
            .collect();
        return stack(
            "vstack",
            "xs",
            Some("stretch"),
            Some("w-full"),
            Some(css(&[("minHeight", format!("{}px", num(height)))])),
            rows,
        );
    }

    let columns = cats
        .iter()
        .enumerate()
        .map(|(cat_idx, label)| {
            let col_style = || css(&[("height", "100%".into())]);
            if stack_mode == "none" {
                let bars: Vec<Value> = series
                    .iter()
                    .enumerate()
                    .map(|(s_idx, s)| {
                        let value = value_at(s, label);
                        let color = series_color(s, s_idx);
                        let class = format!(
                            "rounded-t-sm transition-all duration-slow ease-standard min-h-[4px] cursor-pointer hover:opacity-80 {}",
                            if histogram { "flex-1 mx-0" } else { "flex-1" }
                        );
                        let mut style = vec![("height", format!("{}%", num(value / max * 100.0)))];
                        if !histogram {
                            style.push(("maxWidth", "72px".into()));
                        }
                        style.push(("backgroundColor", color));
                        let mut fields = vec![("className", Value::string(class)), ("style", css(&style))];
                        fields.extend(drill_props(drill, label, value, &s.name));
                        node("box", fields, vec![])
                    })
                    .collect();
                let mut bar_row_fields = vec![
                    ("gap", Value::string(if histogram { "none" } else { "xs" })),
                    ("align", Value::string("end")),
                    ("className", Value::string("w-full flex-1 min-h-0")),
                ];
                if !histogram {
                    bar_row_fields.push(("justify", Value::string("center")));
                }
                let mut cells = vec![node("hstack", bar_row_fields, bars)];
                if show_values && series.len() == 1 {
                    cells.push(typo(num(value_at(&series[0], label)), "caption", Some("secondary"), Some("tabular-nums"), None));
                }
                cells.push(typo(display(label), "caption", Some("secondary"), Some("truncate w-full text-center"), None));
                return node(
                    "vstack",
                    vec![
                        ("gap", Value::string("xs")),
                        ("align", Value::string("center")),
                        ("flex", Value::Bool(true)),
                        ("className", Value::string("min-w-0")),
                        ("style", col_style()),
                    ],
                    cells,
                );
            }
            let bars: Vec<Value> = series
                .iter()
                .enumerate()
                .map(|(s_idx, s)| {
                    let value = value_at(s, label);
                    let color = series_color(s, s_idx);
                    let mut fields = vec![
                        ("className", Value::string("w-full transition-all duration-slow ease-standard cursor-pointer hover:opacity-80")),
                        (
                            "style",
                            css(&[("height", format!("{}%", num(ratio_of(value, cat_idx)))), ("backgroundColor", color)]),
                        ),
                    ];
                    fields.extend(drill_props(drill, label, value, &s.name));
                    node("box", fields, vec![])
                })
                .collect();
            let cells = vec![
                node(
                    "vstack",
                    vec![
                        ("gap", Value::string("none")),
                        ("className", Value::string("w-full flex-1 min-h-0")),
                        ("justify", Value::string("end")),
                    ],
                    bars,
                ),
                typo(display(label), "caption", Some("secondary"), Some("truncate w-full text-center"), None),
            ];
            node(
                "vstack",
                vec![
                    ("gap", Value::string("xs")),
                    ("align", Value::string("center")),
                    ("flex", Value::Bool(true)),
                    ("className", Value::string("min-w-0")),
                    ("style", col_style()),
                ],
                cells,
            )
        })
        .collect();
    stack(
        "hstack",
        if histogram { "none" } else { "xs" },
        Some("end"),
        Some("w-full"),
        Some(css(&[("height", format!("{}px", num(height)))])),
        columns,
    )
}

fn emit_payload(label: &str, value: f64, series_name: &str) -> Value {
    let mut payload = vec![
        ("label", Value::string(label)),
        ("value", Value::Number(value)),
    ];
    if series_name != "default" {
        payload.push(("seriesLabel", Value::string(series_name)));
    }
    object_value(payload)
}

fn pie_chart(
    data: &[Point],
    height: f64,
    show_values: bool,
    donut: bool,
    drill: &Option<String>,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    let total: f64 = data.iter().map(|d| d.value).sum();
    let size = height.min(200.0);
    let radius = size / 2.0 - 8.0;
    let inner_radius = if donut { radius * 0.6 } else { 0.0 };
    let center = size / 2.0;

    struct Segment {
        d: String,
        color: String,
        label: String,
        value: f64,
        percentage: f64,
    }
    let mut segments: Vec<Segment> = Vec::new();
    if total.is_finite() && total > 0.0 {
        let mut current = -std::f64::consts::FRAC_PI_2;
        for (idx, point) in data.iter().enumerate() {
            let angle = point.value / total * 2.0 * std::f64::consts::PI;
            let start = current;
            let end = current + angle;
            current = end;
            let large = if angle > std::f64::consts::PI { 1 } else { 0 };
            let x1 = center + radius * start.cos();
            let y1 = center + radius * start.sin();
            let x2 = center + radius * end.cos();
            let y2 = center + radius * end.sin();
            let full = angle >= 2.0 * std::f64::consts::PI - 1e-9;
            let mid = start + angle / 2.0;
            let xm = center + radius * mid.cos();
            let ym = center + radius * mid.sin();
            let d = if inner_radius > 0.0 {
                let ix1 = center + inner_radius * start.cos();
                let iy1 = center + inner_radius * start.sin();
                let ix2 = center + inner_radius * end.cos();
                let iy2 = center + inner_radius * end.sin();
                if full {
                    let ixm = center + inner_radius * mid.cos();
                    let iym = center + inner_radius * mid.sin();
                    format!("M {ix1} {iy1} L {x1} {y1} A {radius} {radius} 0 1 1 {xm} {ym} A {radius} {radius} 0 1 1 {x2} {y2} L {ix2} {iy2} A {inner_radius} {inner_radius} 0 1 0 {ixm} {iym} A {inner_radius} {inner_radius} 0 1 0 {ix1} {iy1} Z")
                } else {
                    format!("M {ix1} {iy1} L {x1} {y1} A {radius} {radius} 0 {large} 1 {x2} {y2} L {ix2} {iy2} A {inner_radius} {inner_radius} 0 {large} 0 {ix1} {iy1} Z")
                }
            } else if full {
                format!("M {center} {center} L {x1} {y1} A {radius} {radius} 0 1 1 {xm} {ym} A {radius} {radius} 0 1 1 {x2} {y2} Z")
            } else {
                format!(
                    "M {center} {center} L {x1} {y1} A {radius} {radius} 0 {large} 1 {x2} {y2} Z"
                )
            };
            segments.push(Segment {
                d,
                color: point
                    .color
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| CHART_COLORS[idx % CHART_COLORS.len()].to_string()),
                label: point.label.clone(),
                value: point.value,
                percentage: point.value / total * 100.0,
            });
        }
    }

    let vb = format!("0 0 {} {}", num(size), num(size));
    let size_s = num(size);
    let path_class =
        format!("transition-opacity duration-fast hover:opacity-80 cursor-pointer {FOCUS_SVG}");
    let sink = view.sink;
    let total_text = num(total);
    let cx = num(center);
    let svg_el = rsx! {
        svg { width: "{size_s}", height: "{size_s}", view_box: "{vb}",
            for seg in segments.iter() {
                {
                    let drill_event = drill.clone();
                    let payload = emit_payload(&seg.label, seg.value, "default");
                    let aria = drill.as_ref().map(|_| point_name(&seg.label, "default", seg.value));
                    rsx! {
                        path {
                            d: "{seg.d}",
                            fill: "{seg.color}",
                            stroke: "var(--color-card)",
                            stroke_width: "2",
                            class: "{path_class}",
                            "aria-label": aria,
                            onclick: move |_| {
                                if let (Some(s), Some(e)) = (sink, drill_event.clone()) {
                                    s.call((e, payload.clone()));
                                }
                            },
                        }
                    }
                }
            }
            if donut {
                text {
                    x: "{cx}",
                    y: "{cx}",
                    text_anchor: "middle",
                    dominant_baseline: "middle",
                    fill: "var(--color-foreground)",
                    font_size: "14",
                    font_weight: "bold",
                    "{total_text}"
                }
            }
        }
    };
    let legend = show_values.then(|| {
        let rows: Vec<Value> = segments
            .iter()
            .map(|seg| {
                stack(
                    "hstack",
                    "xs",
                    Some("center"),
                    None,
                    None,
                    vec![
                        node(
                            "box",
                            vec![
                                (
                                    "className",
                                    Value::string("w-3 h-3 rounded-sm flex-shrink-0"),
                                ),
                                ("style", css(&[("backgroundColor", seg.color.clone())])),
                            ],
                            vec![],
                        ),
                        typo(
                            format!("{}: {}%", seg.label, fmt_fixed(seg.percentage, 1, 1)),
                            "caption",
                            Some("secondary"),
                            Some("truncate"),
                            None,
                        ),
                    ],
                )
            })
            .collect();
        registry.render_spec(&stack("vstack", "xs", None, None, None, rows), None, view)
    });
    rsx! {
        div { class: "flex flex-row gap-4 items-center justify-center w-full",
            {svg_el}
            if let Some(legend) = legend { {legend} }
        }
    }
}

fn line_chart(
    series: &[Series],
    height: f64,
    show_values: bool,
    fill: bool,
    time_axis: bool,
    period: Option<&str>,
    drill: &Option<String>,
    view: &DioxusView<'_>,
) -> Element {
    let width = 400.0_f64;
    let (pt, pr, pb, pl) = (20.0_f64, 20.0_f64, 30.0_f64, 40.0_f64);
    let chart_width = width - pl - pr;
    let chart_height = height - pt - pb;
    let labels = categories(series);
    let max = {
        let mut m = 1.0_f64;
        for s in series {
            for p in &s.data {
                if p.value > m {
                    m = p.value;
                }
            }
        }
        m
    };
    let x_for = |idx: usize| -> f64 {
        if labels.len() == 1 {
            pl + chart_width / 2.0
        } else {
            pl + (idx as f64 / (labels.len() as f64 - 1.0)) * chart_width
        }
    };
    let y_for = |value: f64| pt + chart_height - (value / max) * chart_height;
    let mut ticks: Vec<f64> = Vec::new();
    for frac in [0.0, 0.5, 1.0] {
        let v = (max * frac).round();
        if ticks.last().copied() != Some(v) {
            ticks.push(v);
        }
    }
    let vb = format!("0 0 {} {}", num(width), num(height));
    let height_s = num(height);
    let sink = view.sink;

    struct Plotted {
        color: String,
        dashed: bool,
        line_path: String,
        area_path: String,
        points: Vec<(f64, f64, f64, String)>,
        name: String,
    }
    let plotted: Vec<Plotted> = series
        .iter()
        .enumerate()
        .map(|(s_idx, s)| {
            let points: Vec<(f64, f64, f64, String)> = labels
                .iter()
                .enumerate()
                .map(|(idx, label)| {
                    let value = s
                        .data
                        .iter()
                        .find(|d| &d.label == label)
                        .map(|d| d.value)
                        .unwrap_or(0.0);
                    (x_for(idx), y_for(value), value, label.clone())
                })
                .collect();
            let line_path = points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "{} {} {}",
                        if i == 0 { "M" } else { "L" },
                        num(p.0),
                        num(p.1)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            let last_x = points.last().map(|p| p.0).unwrap_or(0.0);
            let first_x = points.first().map(|p| p.0).unwrap_or(pl);
            let bottom = pt + chart_height;
            let area_path = format!(
                "{line_path} L {} {} L {} {} Z",
                num(last_x),
                num(bottom),
                num(first_x),
                num(bottom)
            );
            Plotted {
                color: series_color(s, s_idx),
                dashed: s.dashed,
                line_path,
                area_path,
                points,
                name: s.name.clone(),
            }
        })
        .collect();
    let area_opacity = if series.len() > 1 { "0.08" } else { "0.1" };
    let single = series.len() == 1;
    let lone = labels.len() == 1;
    let point_class = format!("cursor-pointer {FOCUS_SVG}");
    rsx! {
        svg { width: "100%", height: "{height_s}", view_box: "{vb}", preserve_aspect_ratio: "xMidYMid meet",
            for frac in [0.0_f64, 0.25, 0.5, 0.75, 1.0] {
                line {
                    x1: num(pl),
                    y1: num(pt + chart_height * (1.0 - frac)),
                    x2: num(width - pr),
                    y2: num(pt + chart_height * (1.0 - frac)),
                    stroke: "var(--color-border)",
                    stroke_dasharray: "4 4",
                    opacity: "0.5",
                }
            }
            for p in plotted.iter() {
                g {
                    if fill {
                        path { d: "{p.area_path}", fill: "{p.color}", opacity: "{area_opacity}" }
                    }
                    path {
                        d: "{p.line_path}",
                        fill: "none",
                        stroke: "{p.color}",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        stroke_dasharray: if p.dashed { Some("6 4") } else { None },
                    }
                    for pt_ in p.points.iter() {
                        g {
                            {
                                let drill_event = drill.clone();
                                let payload = emit_payload(&pt_.3, pt_.2, &p.name);
                                let aria = drill.as_ref().map(|_| point_name(&pt_.3, &p.name, pt_.2));
                                rsx! {
                                    circle {
                                        cx: num(pt_.0),
                                        cy: num(pt_.1),
                                        r: "4",
                                        fill: "var(--color-card)",
                                        stroke: "{p.color}",
                                        stroke_width: "2",
                                        class: "{point_class}",
                                        "aria-label": aria,
                                        onclick: move |_| {
                                            if let (Some(s), Some(e)) = (sink, drill_event.clone()) {
                                                s.call((e, payload.clone()));
                                            }
                                        },
                                    }
                                }
                            }
                            if (show_values || lone) && single {
                                text {
                                    x: num(pt_.0),
                                    y: num(pt_.1 - 10.0),
                                    text_anchor: "middle",
                                    fill: "var(--color-foreground)",
                                    font_size: "10",
                                    font_weight: "500",
                                    {fmt_number(pt_.2)}
                                }
                            }
                        }
                    }
                }
            }
            for v in ticks.iter() {
                text {
                    x: num(pl - 6.0),
                    y: num(y_for(*v) + 3.0),
                    text_anchor: "end",
                    fill: "var(--color-muted-foreground)",
                    font_size: "9",
                    {fmt_number(*v)}
                }
            }
            for (idx, label) in labels.iter().enumerate() {
                {
                    let shown = if time_axis { format_time_label(label, period) } else { label.clone() };
                    rsx! {
                text {
                    x: num(x_for(idx)),
                    y: num(height - 8.0),
                    text_anchor: "middle",
                    fill: "var(--color-muted-foreground)",
                    font_size: "9",
                    "{shown}"
                }
                    }
                }
            }
        }
    }
}

fn scatter_chart(
    data: &[ScatterPoint],
    height: f64,
    drill: &Option<String>,
    view: &DioxusView<'_>,
) -> Element {
    let width = 400.0_f64;
    let (pt, pr, pb, pl) = (20.0_f64, 20.0_f64, 30.0_f64, 40.0_f64);
    let chart_width = width - pl - pr;
    let chart_height = height - pt - pb;
    let (min_x, max_x, min_y, max_y) = if data.is_empty() {
        (0.0, 1.0, 0.0, 1.0)
    } else {
        data.iter().fold(
            (data[0].x, data[0].x, data[0].y, data[0].y),
            |(a, b, c, d), p| (a.min(p.x), b.max(p.x), c.min(p.y), d.max(p.y)),
        )
    };
    let range_x = if max_x - min_x == 0.0 {
        1.0
    } else {
        max_x - min_x
    };
    let range_y = if max_y - min_y == 0.0 {
        1.0
    } else {
        max_y - min_y
    };
    let vb = format!("0 0 {} {}", num(width), num(height));
    let height_s = num(height);
    let sink = view.sink;
    let circle_class = format!("cursor-pointer hover:opacity-100 {FOCUS_SVG}");
    rsx! {
        svg { width: "100%", height: "{height_s}", view_box: "{vb}", preserve_aspect_ratio: "xMidYMid meet",
            for frac in [0.0_f64, 0.25, 0.5, 0.75, 1.0] {
                line {
                    x1: num(pl),
                    y1: num(pt + chart_height * (1.0 - frac)),
                    x2: num(width - pr),
                    y2: num(pt + chart_height * (1.0 - frac)),
                    stroke: "var(--color-border)",
                    stroke_dasharray: "4 4",
                    opacity: "0.5",
                }
            }
            for (idx, p) in data.iter().enumerate() {
                {
                    let cx = pl + ((p.x - min_x) / range_x) * chart_width;
                    let cy = pt + chart_height - ((p.y - min_y) / range_y) * chart_height;
                    let r = p.size.unwrap_or(5.0);
                    let color = p.color.clone().unwrap_or_else(|| CHART_COLORS[idx % CHART_COLORS.len()].to_string());
                    let label = p.label.clone().unwrap_or_else(|| format!("({}, {})", num(p.x), num(p.y)));
                    let drill_event = drill.clone();
                    let payload = emit_payload(&label, p.y, "default");
                    let aria = drill.as_ref().map(|_| point_name(&label, "default", p.y));
                    rsx! {
                        circle {
                            cx: num(cx),
                            cy: num(cy),
                            r: num(r),
                            fill: "{color}",
                            opacity: "0.7",
                            class: "{circle_class}",
                            "aria-label": aria,
                            onclick: move |_| {
                                if let (Some(s), Some(e)) = (sink, drill_event.clone()) {
                                    s.call((e, payload.clone()));
                                }
                            },
                        }
                    }
                }
            }
            text { x: num(pl), y: num(height - 8.0), fill: "var(--color-muted-foreground)", font_size: "9", {fmt_fixed(min_x, 1, 1)} }
            text { x: num(width - pr), y: num(height - 8.0), text_anchor: "end", fill: "var(--color-muted-foreground)", font_size: "9", {fmt_fixed(max_x, 1, 1)} }
        }
    }
}

impl DioxusPatternRenderer for ChartRenderer {
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
        let chart_type = get_string(props, "chartType");
        let resolved_look = get_string(props, "look")
            .or_else(|| {
                chart_type.as_deref().and_then(|t| match t {
                    "bar" => Some("bar-vertical".to_string()),
                    "line" | "pie" | "area" | "donut" | "scatter" | "histogram" => {
                        Some(t.to_string())
                    }
                    _ => None,
                })
            })
            .unwrap_or_else(|| "bar-vertical".to_string());

        if get_bool_or(props, "isLoading", false) {
            let spec = pattern_spec(
                "skeleton",
                [
                    (
                        "spec",
                        props
                            .get("skeleton")
                            .cloned()
                            .unwrap_or_else(|| Value::string("card")),
                    ),
                    ("className", Value::string(class_name)),
                ],
            );
            return registry.render_spec(&spec, None, view);
        }
        if let Some(message) = props
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
        {
            let spec = pattern_spec(
                "error-state",
                [
                    ("title", Value::string("Chart error")),
                    ("message", Value::string(message)),
                    ("className", Value::string(class_name)),
                ],
            );
            return registry.render_spec(&spec, None, view);
        }

        let series_rows = get_array(props, "series");
        let series: Vec<Series> = if !series_rows.is_empty() {
            series_rows
                .iter()
                .map(|s| Series {
                    name: field_text(s, "name").unwrap_or_default(),
                    color: get_string(s, "color"),
                    dashed: get_bool_or(s, "dashed", false),
                    data: parse_points(&get_array(s, "data")),
                })
                .collect()
        } else if props.get("data").and_then(Value::as_array).is_some() {
            vec![Series {
                name: "default".to_string(),
                color: None,
                dashed: false,
                data: parse_points(&get_array(props, "data")),
            }]
        } else {
            vec![]
        };
        let scatter: Vec<ScatterPoint> = get_array(props, "scatterData")
            .iter()
            .map(|p| ScatterPoint {
                x: get_number_or(p, "x", 0.0),
                y: get_number_or(p, "y", 0.0),
                label: field_text(p, "label"),
                size: get_number(p, "size"),
                color: get_string(p, "color"),
            })
            .collect();
        let has_content = if resolved_look == "scatter" {
            !scatter.is_empty()
        } else {
            series.iter().any(|s| !s.data.is_empty())
        };
        if !has_content {
            let spec = pattern_spec(
                "empty-state",
                [
                    ("title", Value::string("No data available")),
                    ("className", Value::string(class_name)),
                ],
            );
            return registry.render_spec(&spec, None, view);
        }

        let title = get_string(props, "title").filter(|t| !t.is_empty());
        let subtitle = get_string(props, "subtitle").filter(|t| !t.is_empty());
        let actions = get_array(props, "actions");
        let height = get_number_or(props, "height", 200.0);
        let show_legend = get_bool_or(props, "showLegend", true);
        let show_values = get_bool_or(props, "showValues", false);
        let stack_mode = get_string_or(props, "stack", "none");
        let time_axis = get_bool_or(props, "timeAxis", false);
        let period = get_string(props, "period");
        let drill = get_string(props, "drillEvent").filter(|e| !e.is_empty());
        let first_series: &[Point] = series.first().map(|s| s.data.as_slice()).unwrap_or(&[]);

        let mut body: Vec<Value> = Vec::new();
        if title.is_some() || subtitle.is_some() || !actions.is_empty() {
            let mut left = Vec::new();
            if let Some(title) = title {
                left.push(typo(title, "h5", None, None, None));
            }
            if let Some(subtitle) = subtitle {
                left.push(typo(subtitle, "small", Some("secondary"), None, None));
            }
            let mut header = vec![stack("vstack", "xs", None, None, None, left)];
            if !actions.is_empty() {
                let badges: Vec<Value> = actions
                    .iter()
                    .map(|action| {
                        let badge = node(
                            "badge",
                            vec![
                                ("variant", Value::string("default")),
                                (
                                    "className",
                                    Value::string(
                                        "cursor-pointer hover:opacity-80 transition-opacity",
                                    ),
                                ),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                            ],
                            vec![],
                        );
                        let mut fields = vec![("className", Value::string("inline-flex"))];
                        if let Some(event) = get_string(action, "event") {
                            fields.push(("action", Value::string(event)));
                        }
                        node("box", fields, vec![badge])
                    })
                    .collect();
                header.push(stack("hstack", "xs", None, None, None, badges));
            }
            body.push(node(
                "hstack",
                vec![
                    ("justify", Value::string("between")),
                    ("align", Value::string("start")),
                ],
                header,
            ));
        }

        let chart_slot = match resolved_look.as_str() {
            "bar-vertical" => Some(bar_chart(
                &series,
                height,
                show_values,
                &stack_mode,
                time_axis,
                period.as_deref(),
                false,
                false,
                &drill,
            )),
            "bar-horizontal" => Some(bar_chart(
                &series,
                height,
                show_values,
                &stack_mode,
                time_axis,
                period.as_deref(),
                false,
                true,
                &drill,
            )),
            "histogram" => Some(bar_chart(
                &series,
                height,
                show_values,
                "none",
                false,
                None,
                true,
                false,
                &drill,
            )),
            _ => None,
        };
        let svg_slot: Option<Element> = match resolved_look.as_str() {
            "line" => Some(line_chart(
                &series,
                height,
                show_values,
                false,
                time_axis,
                period.as_deref(),
                &drill,
                view,
            )),
            "area" => Some(line_chart(
                &series,
                height,
                show_values,
                true,
                time_axis,
                period.as_deref(),
                &drill,
                view,
            )),
            "pie" => Some(pie_chart(
                first_series,
                height,
                show_legend,
                false,
                &drill,
                view,
                registry,
            )),
            "donut" => Some(pie_chart(
                first_series,
                height,
                show_legend,
                true,
                &drill,
                view,
                registry,
            )),
            "scatter" => Some(scatter_chart(&scatter, height, &drill, view)),
            _ => None,
        };
        let top = pattern_spec(
            "vstack",
            [
                ("gap", Value::string("md")),
                ("children", Value::Array(body.into())),
            ],
        );
        let top_el = registry.render_spec(&top, None, view);
        let chart_el = chart_slot.map(|spec| registry.render_spec(&spec, None, view));
        let legend_el = (show_legend && series.len() > 1).then(|| {
            let items: Vec<Value> = series
                .iter()
                .enumerate()
                .map(|(idx, s)| {
                    stack(
                        "hstack",
                        "xs",
                        Some("center"),
                        None,
                        None,
                        vec![
                            node(
                                "box",
                                vec![
                                    (
                                        "className",
                                        Value::string("w-3 h-3 rounded-full flex-shrink-0"),
                                    ),
                                    ("style", css(&[("backgroundColor", series_color(s, idx))])),
                                ],
                                vec![],
                            ),
                            typo(s.name.clone(), "caption", Some("secondary"), None, None),
                        ],
                    )
                })
                .collect();
            let spec = node(
                "hstack",
                vec![
                    ("gap", Value::string("md")),
                    ("justify", Value::string("center")),
                    ("wrap", Value::Bool(true)),
                ],
                items,
            );
            registry.render_spec(&spec, None, view)
        });
        let aria = get_string(props, "aria-label").unwrap_or_else(|| "Chart".to_string());
        let root_class = merge_class_name("p-6 rounded-container", props);
        rsx! {
            div { class: "{root_class}", role: "group", "aria-label": "{aria}",
                div { class: "flex flex-col gap-4 items-stretch justify-start",
                    {top_el}
                    div { class: "w-full",
                        if let Some(el) = chart_el { {el} }
                        if let Some(el) = svg_slot { {el} }
                    }
                    if let Some(el) = legend_el { {el} }
                }
            }
        }
    }
}
