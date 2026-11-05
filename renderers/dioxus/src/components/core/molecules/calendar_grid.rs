//! Native Dioxus body for the `calendar-grid` pattern, ported from web `CalendarGrid.tsx`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

pub struct CalendarGridRenderer;

impl CalendarGridRenderer {
    const NAME: &str = "calendar-grid";
}

#[derive(Props, Clone, PartialEq)]
struct GridProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const DEFAULT_FIRST_HOUR: i32 = 9;
const DEFAULT_LAST_HOUR: i32 = 17;

#[derive(Clone, PartialEq, PartialOrd)]
struct Moment {
    date: String,
    seconds: u32,
}

fn node(name: &str, mut fields: Vec<(&str, Value)>, children: Vec<Value>) -> Value {
    if !children.is_empty() {
        fields.push(("children", Value::Array(children.into())));
    }
    pattern_spec(name, fields)
}

fn parse_moment(raw: &Value) -> Option<Moment> {
    let text = match raw {
        Value::String(text) => text.to_string(),
        _ => return None,
    };
    let date = text.get(..10)?;
    let bytes = date.as_bytes();
    let well_formed = bytes.iter().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            *b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    if !well_formed {
        return None;
    }
    let mut seconds = 0;
    if text.len() > 10 {
        if !text[10..].starts_with('T') {
            return None;
        }
        let mut parts = text[11..]
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty());
        for factor in [3600, 60, 1] {
            if let Some(part) = parts.next().and_then(|p| p.parse::<u32>().ok()) {
                seconds += part * factor;
            }
        }
    }
    Some(Moment {
        date: date.to_string(),
        seconds,
    })
}

fn field<'a>(row: &'a Value, name: &str) -> Option<&'a Value> {
    row.get_path(name)
}

fn start_of(event: &Value, start_field: &str) -> Option<Moment> {
    field(event, start_field).and_then(parse_moment)
}

fn end_of(event: &Value, end_field: &str) -> Option<Moment> {
    field(event, end_field).and_then(parse_moment)
}

fn slot_hour(slot: &str) -> Option<u32> {
    slot.split(':').next()?.parse().ok()
}

fn event_in_slot(event: &Value, day_iso: &str, slot: &str, start_field: &str) -> bool {
    let (Some(start), Some(hour)) = (start_of(event, start_field), slot_hour(slot)) else {
        return false;
    };
    start.date == day_iso && start.seconds / 3600 == hour
}

fn event_continues(
    event: &Value,
    day_iso: &str,
    slot: &str,
    start_field: &str,
    end_field: &str,
) -> bool {
    let (Some(start), Some(end), Some(hour)) = (
        start_of(event, start_field),
        end_of(event, end_field),
        slot_hour(slot),
    ) else {
        return false;
    };
    let slot_start = Moment {
        date: day_iso.to_string(),
        seconds: hour * 3600,
    };
    slot_start > start && slot_start < end
}

fn default_slots(events: &[Value], start_field: &str, end_field: &str) -> Vec<String> {
    let mut first = DEFAULT_FIRST_HOUR;
    let mut last = DEFAULT_LAST_HOUR;
    for event in events {
        let Some(start) = start_of(event, start_field) else {
            continue;
        };
        let hour = (start.seconds / 3600) as i32;
        first = first.min(hour);
        last = last.max(hour);
        if let Some(end) = end_of(event, end_field) {
            if end.date == start.date {
                let end_hour = if end.seconds % 3600 == 0 {
                    (end.seconds / 3600) as i32 - 1
                } else {
                    (end.seconds / 3600) as i32
                };
                last = last.max(end_hour);
            }
        }
    }
    (first..=last).map(|hour| format!("{hour:02}:00")).collect()
}

fn text_of(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Int(n)) => n.to_string(),
        Some(Value::Number(n)) => format!("{n}"),
        _ => String::new(),
    }
}

fn monday_of(start: &str) -> String {
    let mut date = calendar_week_start(Some(start));
    while calendar_days(date, 1)[0].weekday != "Mon" {
        match date.pred_opt() {
            Some(previous) => date = previous,
            None => break,
        }
    }
    calendar_days(date, 1)[0].iso.clone()
}

fn week_days(props: &Value) -> Vec<CalendarDay> {
    let start = match get_string(props, "weekStart") {
        Some(raw) => monday_of(&raw),
        None => calendar_days(calendar_week_start(None), 1)[0].iso.clone(),
    };
    calendar_days(calendar_week_start(Some(&start)), 7)
}

fn date_range(first: &CalendarDay, last: &CalendarDay) -> String {
    if first.iso == last.iso {
        first.label.clone()
    } else {
        format!("{} – {}", first.label, last.label)
    }
}

impl DioxusPatternRenderer for CalendarGridRenderer {
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
        rsx! { CalendarGridView { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[allow(non_snake_case)]
fn CalendarGridView(input: GridProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let offset = use_signal(|| 0usize);

    let events = get_array(props, "events");
    let title_field = get_string_or(props, "titleField", "title");
    let start_field = get_string_or(props, "startField", "startTime");
    let end_field = get_string_or(props, "endField", "endTime");
    let color_field = get_string_or(props, "colorField", "color");
    let on_slot = get_string(props, "onSlotClick");
    let on_day = get_string(props, "onDayClick");
    let on_event = get_string(props, "onEventClick");

    let all_days = week_days(props);
    let visible_count = calendar_day_window(props);
    let clamped = offset().min(7usize.saturating_sub(visible_count));
    let visible: Vec<&CalendarDay> = all_days.iter().skip(clamped).take(visible_count).collect();
    let can_prev = clamped > 0;
    let can_next = clamped + visible_count < 7;
    let slots = if props.get("timeSlots").is_some() {
        get_string_array(props, "timeSlots")
    } else {
        default_slots(&events, &start_field, &end_field)
    };
    let today = calendar_day(None).iso;
    let grid_cols = match visible_count {
        1 => "grid-cols-2",
        3 => "grid-cols-4",
        _ => "grid-cols-8",
    };

    let event_press = |event: &Value| -> Vec<(&'static str, Value)> {
        match &on_event {
            Some(name) => vec![
                ("action", Value::string(name.clone())),
                ("actionPayload", event.clone()),
            ],
            None => vec![],
        }
    };

    let render_event = |event: &Value| -> Value {
        let color = field(event, &color_field)
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .map(str::to_string);
        let label = text_of(field(event, &title_field));
        let class = format!(
            "cursor-pointer hover:shadow-elevation-card transition-shadow text-xs truncate {}",
            color
                .clone()
                .unwrap_or_else(|| "bg-primary/10 border-primary/30 text-foreground".to_string())
        );
        let mut fields = vec![
            ("rounded", Value::string("md")),
            ("padding", Value::string("xs")),
            ("border", Value::Bool(true)),
            ("className", Value::string(class)),
        ];
        fields.extend(event_press(event));
        node(
            "box",
            fields,
            vec![node(
                "typography",
                vec![
                    ("variant", Value::string("small")),
                    ("className", Value::string("truncate font-medium")),
                    ("content", Value::string(label)),
                ],
                vec![],
            )],
        )
    };

    let render_continuation = |event: &Value| -> Value {
        let color = field(event, &color_field)
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .map(str::to_string);
        let class = match color {
            Some(color) => format!("cursor-pointer h-2 {color} opacity-50"),
            None => "cursor-pointer h-2 bg-primary/10 border-primary/20".to_string(),
        };
        let mut fields = vec![
            ("rounded", Value::string("sm")),
            ("border", Value::Bool(true)),
            ("className", Value::string(class)),
        ];
        if on_event.is_some() {
            fields.push((
                "aria-label",
                Value::string(text_of(field(event, &title_field))),
            ));
        }
        fields.extend(event_press(event));
        node("box", fields, vec![])
    };

    let header_cells: Vec<Value> = std::iter::once(node(
        "box",
        vec![("className", Value::string("p-2"))],
        vec![],
    ))
    .chain(visible.iter().map(|day| {
        let count = events
            .iter()
            .filter(|event| start_of(event, &start_field).is_some_and(|m| m.date == day.iso))
            .count();
        let mut day_cell = vec![
            ("date", Value::string(day.iso.clone())),
            ("isToday", Value::Bool(day.iso == today)),
        ];
        if let Some(name) = &on_day {
            day_cell.push(("onClick", Value::string(name.clone())));
        }
        let mut children = vec![node("day-cell", day_cell, vec![])];
        if count > 0 {
            children.push(node(
                "box",
                vec![("className", Value::string("text-center pb-1"))],
                vec![node(
                    "badge",
                    vec![
                        ("variant", Value::string("default")),
                        ("size", Value::string("sm")),
                        ("label", Value::Int(count as i64)),
                    ],
                    vec![],
                )],
            ));
        }
        node(
            "box",
            vec![("className", Value::string("border-l border-border"))],
            children,
        )
    }))
    .collect();

    let slot_rows: Vec<Value> = slots
        .iter()
        .map(|time| {
            let mut cells = vec![node(
                "box",
                vec![("className", Value::string("p-2 text-right pr-3"))],
                vec![node(
                    "typography",
                    vec![
                        ("variant", Value::string("small")),
                        ("className", Value::string("text-muted-foreground")),
                        ("content", Value::string(time.clone())),
                    ],
                    vec![],
                )],
            )];
            for day in visible.iter() {
                let slot_events: Vec<&Value> = events
                    .iter()
                    .filter(|event| event_in_slot(event, &day.iso, time, &start_field))
                    .collect();
                let continuing: Vec<&Value> = events
                    .iter()
                    .filter(|event| {
                        event_continues(event, &day.iso, time, &start_field, &end_field)
                    })
                    .collect();
                let mut inner: Vec<Value> = slot_events.iter().map(|e| render_event(e)).collect();
                inner.extend(continuing.iter().map(|e| render_continuation(e)));
                let class = if day.iso == today {
                    "border-l border-border bg-primary/10"
                } else {
                    "border-l border-border"
                };
                let mut fields = vec![
                    ("time", Value::string(time.clone())),
                    (
                        "isOccupied",
                        Value::Bool(!slot_events.is_empty() || !continuing.is_empty()),
                    ),
                    ("className", Value::string(class)),
                ];
                if let Some(name) = &on_slot {
                    fields.push(("onClick", Value::string(name.clone())));
                    fields.push((
                        "actionPayload",
                        object_value([
                            ("date", Value::string(day.iso.clone())),
                            ("time", Value::string(time.clone())),
                        ]),
                    ));
                }
                cells.push(node(
                    "time-slot-cell",
                    fields,
                    vec![node("vstack", vec![("gap", Value::string("xs"))], inner)],
                ));
            }
            node(
                "box",
                vec![(
                    "className",
                    Value::string(format!("grid border-b border-border {grid_cols}")),
                )],
                cells,
            )
        })
        .collect();

    let body = node(
        "box",
        vec![],
        vec![
            node(
                "box",
                vec![(
                    "className",
                    Value::string(format!("grid border-b border-border {grid_cols}")),
                )],
                header_cells,
            ),
            node(
                "box",
                vec![("className", Value::string("max-h-[500px] overflow-y-auto"))],
                slot_rows,
            ),
        ],
    );
    let body_el = registry.render_spec(&body, None, &view);

    let pager = (visible_count < 7).then(|| {
        let prev = node(
            "button",
            vec![
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("chevron-left")),
                ("aria-label", Value::string("Previous days")),
                ("label", Value::string("Previous")),
                ("action", Value::string("__local_prev")),
            ],
            vec![],
        );
        let next = node(
            "button",
            vec![
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("iconRight", Value::string("chevron-right")),
                ("aria-label", Value::string("Next days")),
                ("label", Value::string("Next")),
                ("action", Value::string("__local_next")),
            ],
            vec![],
        );
        let range = node(
            "typography",
            vec![
                ("variant", Value::string("small")),
                ("className", Value::string("text-muted-foreground")),
                (
                    "content",
                    Value::string(match (visible.first(), visible.last()) {
                        (Some(first), Some(last)) => date_range(first, last),
                        _ => String::new(),
                    }),
                ),
            ],
            vec![],
        );
        let prev_el =
            crate::renderer::render_with_local_sink(&view, registry, &prev, move |name, _| {
                if name != "__local_prev" {
                    return false;
                }
                if can_prev {
                    let mut state = offset;
                    state.set(clamped.saturating_sub(visible_count));
                }
                true
            });
        let range_el = registry.render_spec(&range, None, &view);
        let next_el =
            crate::renderer::render_with_local_sink(&view, registry, &next, move |name, _| {
                if name != "__local_next" {
                    return false;
                }
                if can_next {
                    let mut state = offset;
                    state.set((clamped + visible_count).min(7 - visible_count));
                }
                true
            });
        rsx! {
            div { class: "flex flex-row gap-4 items-center justify-between mb-2 px-2",
                {prev_el}
                {range_el}
                {next_el}
            }
        }
    });

    let root_class = merge_class_name("", props);
    rsx! {
        div { class: "{root_class}",
            if let Some(pager) = pager { {pager} }
            {body_el}
        }
    }
}
