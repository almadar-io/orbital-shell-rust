//! Native Dioxus body for the `gantt` pattern, mirroring `Gantt.tsx`.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `gantt` pattern.
pub struct GanttRenderer;

impl GanttRenderer {
    const NAME: &str = "gantt";
}

impl DioxusPatternRenderer for GanttRenderer {
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
        fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
            let y = if m <= 2 { y - 1 } else { y };
            let era = (if y >= 0 { y } else { y - 399 }) / 400;
            let yoe = y - era * 400;
            let mp = (m + 9) % 12;
            let doy = (153 * mp + 2) / 5 + d - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            era * 146097 + doe - 719468
        }
        fn day_of_month(z: i64) -> i64 {
            let z = z + 719468;
            let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
            let doe = z - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            doy - (153 * mp + 2) / 5 + 1
        }
        fn parse_day(v: Option<&Value>) -> Option<i64> {
            match v {
                Some(n @ (Value::Number(_) | Value::Int(_))) => {
                    n.as_number().map(|n| (n / 86_400_000.0).floor() as i64)
                }
                Some(Value::String(s)) => {
                    let t = s.trim();
                    let b = t.as_bytes();
                    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
                        return None;
                    }
                    let y = t.get(0..4)?.parse::<i64>().ok()?;
                    let m = t.get(5..7)?.parse::<i64>().ok()?;
                    let d = t.get(8..10)?.parse::<i64>().ok()?;
                    Some(days_from_civil(y, m, d))
                }
                _ => None,
            }
        }
        fn nested<'a>(row: &'a Value, path: &str) -> Option<&'a Value> {
            let mut cur = row;
            for seg in path.split('.') {
                cur = cur.as_object()?.get(seg)?;
            }
            Some(cur)
        }
        fn text_of(v: Option<&Value>) -> String {
            match v {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone().into_inner(),
                Some(other) => format_value(other),
            }
        }
        let label_w = 192.0_f64;
        let row_h = 36.0_f64;
        let head_h = 44.0_f64;
        let title_field = get_string_or(props, "titleField", "title");
        let start_field = get_string_or(props, "startField", "start");
        let end_field = get_string_or(props, "endField", "end");
        let status_field = get_string_or(props, "statusField", "status");
        let group_field = get_string_or(props, "groupField", "");
        let duration_field = get_string(props, "durationField").filter(|f| !f.is_empty());
        let day_w = get_number_or(props, "dayWidth", 28.0);
        let click_event = get_string(props, "barClickEvent");
        let sink = view.sink;
        let error_message = props
            .as_object()
            .and_then(|o| o.get("error"))
            .filter(|e| !matches!(e, Value::Null))
            .map(|e| get_string_or(e, "message", ""));
        let loading = get_bool_or(props, "isLoading", false);
        let color_map = props
            .as_object()
            .and_then(|o| o.get("statusColorMap"))
            .cloned()
            .unwrap_or(Value::Null);
        let explicit = get_array(props, "tasks");
        let rows = if explicit.is_empty() {
            get_entity_rows_from(props, view.entities)
        } else {
            explicit
        };
        let mut placed: Vec<(String, String, String, String, i64, i64, bool)> = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            let declared_end = parse_day(nested(row, &end_field));
            let Some(declared_start) = parse_day(nested(row, &start_field)).or(declared_end) else {
                continue;
            };
            let (start, end, reversed) = match declared_end {
                Some(e) => {
                    let reversed = e < declared_start;
                    if reversed {
                        (e, declared_start + 1, true)
                    } else {
                        (declared_start, e + 1, false)
                    }
                }
                None => {
                    let days = duration_field
                        .as_ref()
                        .and_then(|f| nested(row, f))
                        .and_then(|v| match v {
                            Value::Number(_) | Value::Int(_) => v.as_number(),
                            Value::String(s) => s.trim().parse::<f64>().ok(),
                            _ => None,
                        });
                    let span = match days {
                        Some(d) if d.is_finite() && d > 0.0 => d.round() as i64,
                        _ => 1,
                    };
                    (declared_start, declared_start + span, false)
                }
            };
            let id = row
                .as_object()
                .and_then(|o| o.get("id"))
                .map(format_value)
                .unwrap_or_else(|| idx.to_string());
            let group = if group_field.is_empty() {
                String::new()
            } else {
                text_of(nested(row, &group_field))
            };
            placed.push((
                id,
                text_of(nested(row, &title_field)),
                text_of(nested(row, &status_field)),
                group,
                start,
                end,
                reversed,
            ));
        }
        let class_name = get_string_or(props, "className", "");
        if loading {
            let spec = pattern_spec(
                "loading-state",
                [
                    ("message", Value::string(crate::i18n::t("common.loading"))),
                    ("className", Value::string(class_name.clone())),
                ],
            );
            return registry.render_spec(&spec, None, view);
        }
        if let Some(message) = error_message {
            let class = cn(&[BOX_ATOM_BASE, "p-4", class_name.trim()]);
            let text = pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body")),
                    ("color", Value::string("error")),
                    ("content", Value::string(message)),
                ],
            );
            return rsx! {
                div { class: "{class}", ..shared_root_attrs(props), {registry.render_spec(&text, None, view)} }
            };
        }
        if placed.is_empty() {
            let spec = pattern_spec(
                "empty-state",
                [
                    ("title", Value::string(crate::i18n::t("empty.noData"))),
                    ("className", Value::string(class_name.clone())),
                ],
            );
            return registry.render_spec(&spec, None, view);
        }
        let axis_start = parse_day(props.as_object().and_then(|o| o.get("rangeStart")))
            .unwrap_or_else(|| placed.iter().map(|p| p.4).min().unwrap_or(0) - 2);
        let axis_end_raw = parse_day(props.as_object().and_then(|o| o.get("rangeEnd")))
            .unwrap_or_else(|| placed.iter().map(|p| p.5).max().unwrap_or(0) + 2);
        let axis_end = if axis_end_raw > axis_start {
            axis_end_raw
        } else {
            axis_start + 1
        };
        let total_days = axis_end - axis_start;
        let chart_w = total_days as f64 * day_w;
        let offset = |d: i64| -> f64 { (d - axis_start) as f64 * day_w };
        let mut items: Vec<(bool, String, String, f64, f64, String, bool)> = Vec::new();
        let mut seen_groups: Vec<String> = Vec::new();
        for (id, title, status, group, start, end, reversed) in placed.iter() {
            if !group_field.is_empty() && !seen_groups.contains(group) {
                seen_groups.push(group.clone());
                items.push((
                    true,
                    if group.is_empty() {
                        "—".to_string()
                    } else {
                        group.clone()
                    },
                    String::new(),
                    0.0,
                    0.0,
                    String::new(),
                    false,
                ));
            }
            let declared = color_map
                .as_object()
                .and_then(|m| m.get(status))
                .and_then(|c| c.as_str())
                .unwrap_or("default");
            let bar_cls = if *reversed {
                "bg-error/30 border border-dashed border-error"
            } else {
                match declared {
                    "secondary" => "bg-secondary/80 hover:bg-secondary",
                    "success" => "bg-success/80 hover:bg-success",
                    "warning" => "bg-warning/80 hover:bg-warning",
                    "danger" | "destructive" | "error" => "bg-error/80 hover:bg-error",
                    "info" => "bg-info/80 hover:bg-info",
                    "neutral" => "bg-muted-foreground/50 hover:bg-muted-foreground/70",
                    _ => "bg-primary/80 hover:bg-primary",
                }
            };
            items.push((
                false,
                title.clone(),
                id.clone(),
                offset(*start),
                offset(*end) - offset(*start),
                bar_cls.to_string(),
                *reversed,
            ));
        }
        let geometry: Vec<(String, f64, f64, f64)> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| !it.0)
            .map(|(i, it)| {
                (
                    it.2.clone(),
                    it.3,
                    (it.3 + it.4).max(it.3 + day_w / 2.0),
                    head_h + i as f64 * row_h + row_h / 2.0,
                )
            })
            .collect();
        let link_paths: Vec<(String, String)> = get_array(props, "links")
            .iter()
            .filter_map(|l| {
                let from = geometry
                    .iter()
                    .find(|g| g.0 == get_string_or(l, "from", ""))?;
                let to = geometry
                    .iter()
                    .find(|g| g.0 == get_string_or(l, "to", ""))?;
                let mid = from.2 + 8.0_f64.max((to.1 - from.2) / 2.0);
                let d = format!(
                    "M {} {} L {} {} L {} {} L {} {}",
                    from.2, from.3, mid, from.3, mid, to.3, to.1, to.3
                );
                let head = format!(
                    "{},{} {},{} {},{}",
                    to.1 - 8.0,
                    to.3 - 4.0,
                    to.1,
                    to.3,
                    to.1 - 8.0,
                    to.3 + 4.0
                );
                Some((d, head))
            })
            .collect();
        let days: Vec<(i64, bool, String)> = (0..total_days)
            .map(|i| {
                let z = axis_start + i;
                let weekday = (z + 4).rem_euclid(7);
                (i, weekday == 0 || weekday == 6, day_of_month(z).to_string())
            })
            .collect();
        let today = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .ok()
            .map(|elapsed| (elapsed.as_secs() / 86_400) as i64)
            .filter(|today| *today >= axis_start && *today <= axis_end)
            .map(|today| offset(today));
        let show_today = get_bool_or(props, "showToday", true);
        let hstack = "flex flex-row gap-0 items-stretch justify-start";
        let surface = if get_string_or(props, "surface", "auto") == "auto" {
            "surface-content"
        } else {
            ""
        };
        let root_class = cn(&[BOX_ATOM_BASE, "w-full overflow-auto", surface, class_name.trim()]);
        let inner_class = cn(&[BOX_ATOM_BASE, "relative"]);
        let inner_style = format!("width: {}px; min-width: 100%", label_w + chart_w);
        let head_class = cn(&[hstack, "sticky top-0 z-20 bg-card border-b border-border"]);
        let head_style = format!("height: {head_h}px");
        let head_label_class = cn(&[BOX_ATOM_BASE, "sticky left-0 z-10 shrink-0 bg-card border-r border-border"]);
        let head_label_style = format!("width: {label_w}px; height: {head_h}px");
        let head_chart_style = format!("width: {chart_w}px; height: {head_h}px");
        let rows_class = cn(&["flex flex-col gap-0 items-stretch justify-start", "relative"]);
        let group_row_class = cn(&[hstack, "border-b border-border bg-muted/30"]);
        let group_label_class = cn(&[BOX_ATOM_BASE, "sticky left-0 z-10 shrink-0 bg-muted/30 px-3 flex items-center border-r border-border"]);
        let task_row_class = cn(&[hstack, "border-b border-border/50"]);
        let task_label_class = cn(&[BOX_ATOM_BASE, "sticky left-0 z-10 shrink-0 bg-card px-3 flex items-center border-r border-border"]);
        let plain_box = cn(&[BOX_ATOM_BASE]);
        let relative_box = cn(&[BOX_ATOM_BASE, "relative"]);
        let row_style = format!("height: {row_h}px");
        let label_cell_style = format!("width: {label_w}px; height: {row_h}px");
        let chart_cell_style = format!("width: {chart_w}px; height: {row_h}px");
        let svg_style = format!("left: {label_w}px; top: 0px");
        let svg_h = head_h + items.len() as f64 * row_h;
        let show_numbers = day_w >= 20.0;
        let today_class = cn(&[BOX_ATOM_BASE, "absolute top-0 bottom-0 w-0.5 bg-error/70 pointer-events-none"]);
        let today_style = today.map(|offset| format!("left: {}px", label_w + offset));
        let attrs = shared_root_attrs(props);
        rsx! {
            div { class: "{root_class}", ..attrs,
                div { class: "{inner_class}", style: "{inner_style}",
                    div { class: "{head_class}", style: "{head_style}",
                        div { class: "{head_label_class}", style: "{head_label_style}" }
                        div { class: "{relative_box}", style: "{head_chart_style}",
                            for (i, weekend, number) in days.iter() {
                                {
                                    let day_class = cn(&[
                                        BOX_ATOM_BASE,
                                        "absolute top-0 bottom-0 border-l border-border/50 flex items-end justify-center pb-1",
                                        if *weekend { "bg-muted/40" } else { "" },
                                    ]);
                                    let day_style = format!("left: {}px; width: {day_w}px", *i as f64 * day_w);
                                    let number_spec = pattern_spec(
                                        "typography",
                                        [
                                            ("variant", Value::string("caption")),
                                            ("color", Value::string("secondary")),
                                            ("content", Value::string(number.to_string())),
                                        ],
                                    );
                                    rsx! {
                                        div { key: "{i}", class: "{day_class}", style: "{day_style}",
                                            if show_numbers { {registry.render_spec(&number_spec, None, view)} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "{rows_class}",
                        for (idx, (is_group, label, id, left, width, bar_cls, reversed)) in items.iter().enumerate() {
                            if *is_group {
                                {
                                    let label_spec = pattern_spec(
                                        "typography",
                                        [
                                            ("variant", Value::string("caption")),
                                            ("weight", Value::string("semibold")),
                                            ("content", Value::string(label.to_string())),
                                        ],
                                    );
                                    rsx! {
                                        div { key: "g-{idx}", class: "{group_row_class}", style: "{row_style}",
                                            div { class: "{group_label_class}", style: "{label_cell_style}",
                                                {registry.render_spec(&label_spec, None, view)}
                                            }
                                            div { class: "{plain_box}", style: "{chart_cell_style}" }
                                        }
                                    }
                                }
                            } else {
                                {
                                    let ev = click_event.clone();
                                    let bar_id = id.clone();
                                    let bar_style = format!("left: {left}px; width: {width}px");
                                    let bar_class = cn(&[
                                        BOX_ATOM_BASE,
                                        if click_event.is_some() { "cursor-pointer" } else { "" },
                                        "absolute top-1/2 -translate-y-1/2 h-4 rounded-sm transition-colors",
                                        bar_cls,
                                    ]);
                                    let tip = if *reversed { Some(crate::i18n::t("gantt.datesReversed")) } else { None };
                                    let reversed_attr = if *reversed { Some("true") } else { None };
                                    let role = if click_event.is_some() { Some("button") } else { None };
                                    let tab_index = if click_event.is_some() { Some("0") } else { None };
                                    let label_spec = pattern_spec(
                                        "typography",
                                        [
                                            ("variant", Value::string("small")),
                                            ("className", Value::string("truncate")),
                                            ("content", Value::string(label.to_string())),
                                        ],
                                    );
                                    rsx! {
                                        div { key: "{id}", class: "{task_row_class}", style: "{row_style}",
                                            div { class: "{task_label_class}", style: "{label_cell_style}",
                                                {registry.render_spec(&label_spec, None, view)}
                                            }
                                            div { class: "{relative_box}", style: "{chart_cell_style}",
                                                div {
                                                    class: "{bar_class}",
                                                    style: "{bar_style}",
                                                    "data-gantt-bar": "{id}",
                                                    "data-dates-reversed": reversed_attr,
                                                    title: tip,
                                                    role: role,
                                                    tabindex: tab_index,
                                                    onclick: move |_| {
                                                        if let (Some(s), Some(e)) = (sink, ev.clone()) {
                                                            let mut payload = orbital_core::ValueMap::new();
                                                            payload.insert("id".to_string().into(), Value::String(bar_id.clone().into()));
                                                            s.call((e, Value::Object(payload.into())));
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if !link_paths.is_empty() {
                            svg { class: "absolute pointer-events-none text-muted-foreground", style: "{svg_style}", width: "{chart_w}", height: "{svg_h}",
                                for (i, (d, head)) in link_paths.iter().enumerate() {
                                    g { key: "{i}",
                                        path { d: "{d}", fill: "none", stroke: "currentColor", stroke_width: "1.5" }
                                        polygon { points: "{head}", fill: "currentColor" }
                                    }
                                }
                            }
                        }
                        if show_today {
                            if let Some(style) = today_style {
                                div { class: "{today_class}", style: "{style}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
