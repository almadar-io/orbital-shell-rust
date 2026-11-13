use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, DioxusPatternRenderer,
    DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use web_time::{SystemTime, UNIX_EPOCH};

/// Dioxus renderer for the `date-range-picker` pattern.
pub struct DateRangePickerRenderer;

#[derive(Props, Clone, PartialEq)]
struct DateRangePickerProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const FROM: &str = "__date_range_from";
const TO: &str = "__date_range_to";
const PRESET: &str = "__date_range_preset:";

const DEFAULT_PRESETS: [(&str, &str); 5] = [
    ("7d", "Last 7 days"),
    ("30d", "Last 30 days"),
    ("month", "This Month"),
    ("quarter", "This Quarter"),
    ("ytd", "YTD"),
];

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn iso(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn token_range(token: &str) -> Option<(String, String)> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let today = (now.as_secs() / 86400) as i64;
    let (year, month, _) = civil_from_days(today);
    let from = match token {
        "7d" => today - 7,
        "30d" => today - 30,
        "month" => days_from_civil(year, month, 1),
        "quarter" => days_from_civil(year, (month - 1) / 3 * 3 + 1, 1),
        "ytd" => days_from_civil(year, 1, 1),
        _ => return None,
    };
    Some((iso(from), iso(today)))
}

#[derive(Clone)]
struct Preset {
    label: String,
    value: String,
    range: Option<(String, String)>,
}

impl Preset {
    fn resolve(&self) -> Option<(String, String)> {
        self.range.clone().or_else(|| token_range(&self.value))
    }
}

fn presets(props: &Value) -> Vec<Preset> {
    let declared = props
        .as_object()
        .and_then(|fields| fields.get("presets"))
        .and_then(Value::as_array);
    let all: Vec<Preset> = match declared {
        Some(items) => items
            .iter()
            .map(|item| Preset {
                label: get_string_or(item, "label", ""),
                value: get_string_or(item, "value", ""),
                range: item
                    .get("range")
                    .and_then(|range| Some((get_string(range, "from")?, get_string(range, "to")?))),
            })
            .collect(),
        None => DEFAULT_PRESETS
            .iter()
            .map(|(value, label)| Preset {
                label: label.to_string(),
                value: value.to_string(),
                range: None,
            })
            .collect(),
    };
    all.into_iter()
        .filter(|preset| {
            preset.range.is_some()
                || DEFAULT_PRESETS
                    .iter()
                    .any(|(value, _)| *value == preset.value)
        })
        .collect()
}

#[allow(non_snake_case)]
fn DateRangePicker(input: DateRangePickerProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let initial_from = get_string_or(props, "from", "");
    let initial_to = get_string_or(props, "to", "");
    let mut from = use_signal(move || initial_from);
    let mut to = use_signal(move || initial_to);
    let mut active = use_signal(|| None::<String>);
    let event = get_string(props, "event").filter(|name| !name.is_empty());
    let sink = view.sink;
    let preset_list = presets(props);
    let relay_presets = preset_list.clone();

    let relay: EventSink = Callback::new(move |(name, payload): (String, Value)| {
        let emit = |from: &str, to: &str| {
            if let (Some(sink), Some(event)) = (sink, &event) {
                sink.call((
                    event.clone(),
                    object_value([("from", Value::string(from)), ("to", Value::string(to))]),
                ));
            }
        };
        if name == FROM {
            if let Some(next) = get_string(&payload, "value") {
                from.set(next.clone());
                active.set(None);
                emit(&next, &to.peek());
            }
        } else if name == TO {
            if let Some(next) = get_string(&payload, "value") {
                to.set(next.clone());
                active.set(None);
                emit(&from.peek(), &next);
            }
        } else if let Some(index) = name
            .strip_prefix(PRESET)
            .and_then(|index| index.parse::<usize>().ok())
        {
            let Some(preset) = relay_presets.get(index) else {
                return;
            };
            let Some((next_from, next_to)) = preset.resolve() else {
                return;
            };
            from.set(next_from.clone());
            to.set(next_to.clone());
            active.set(Some(preset.value.clone()));
            emit(&next_from, &next_to);
        }
    });

    let field = |label_key: &str, default_label: &str, action: &str, value: String| {
        pattern_spec(
            "vstack",
            [
                ("gap", Value::string("xs")),
                (
                    "children",
                    array_value(vec![
                        pattern_spec(
                            "typography",
                            [
                                ("variant", Value::string("caption")),
                                ("color", Value::string("secondary")),
                                (
                                    "content",
                                    Value::string(get_string_or(props, label_key, default_label)),
                                ),
                            ],
                        ),
                        pattern_spec(
                            "input",
                            [
                                ("inputType", Value::string("date")),
                                ("value", Value::string(value)),
                                ("onChange", Value::string(action)),
                            ],
                        ),
                    ]),
                ),
            ],
        )
    };
    let mut children = vec![pattern_spec(
        "hstack",
        [
            ("gap", Value::string("md")),
            ("align", Value::string("end")),
            (
                "children",
                array_value(vec![
                    field("fromLabel", &crate::i18n::t("dateRange.from"), FROM, from()),
                    field("toLabel", &crate::i18n::t("dateRange.to"), TO, to()),
                ]),
            ),
        ],
    )];
    if !preset_list.is_empty() {
        let current = active();
        let buttons = preset_list
            .iter()
            .enumerate()
            .map(|(index, preset)| {
                let is_active = current.as_deref() == Some(preset.value.as_str());
                pattern_spec(
                    "button",
                    [
                        (
                            "variant",
                            Value::string(if is_active { "primary" } else { "ghost" }),
                        ),
                        ("size", Value::string("sm")),
                        ("label", Value::string(preset.label.clone())),
                        ("action", Value::string(format!("{PRESET}{index}"))),
                    ],
                )
            })
            .collect();
        children.push(pattern_spec(
            "hstack",
            [
                ("gap", Value::string("xs")),
                ("wrap", Value::Bool(true)),
                ("children", array_value(buttons)),
            ],
        ));
    }
    let mut root = vec![
        ("gap".to_string(), Value::string("sm")),
        (
            "className".to_string(),
            Value::string(cn(&[get_string(props, "className")
                .as_deref()
                .unwrap_or("")])),
        ),
        ("children".to_string(), array_value(children)),
    ];
    root.extend(dom_passthrough(props));
    let inner_view = view.for_source(view.source_trait.clone(), relay);
    input
        .registry
        .render_spec(&pattern_spec("vstack", root), None, &inner_view)
}

impl DioxusPatternRenderer for DateRangePickerRenderer {
    fn pattern_name(&self) -> &str {
        "date-range-picker"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { DateRangePicker { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
