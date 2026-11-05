//! Native Dioxus body for the `likert-scale` pattern, mirroring `LikertScale.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct LikertScaleRenderer;

const DEFAULT_OPTIONS: [(i64, &str); 5] = [
    (1, "Strongly Disagree"),
    (2, "Disagree"),
    (3, "Neutral"),
    (4, "Agree"),
    (5, "Strongly Agree"),
];

fn same(left: &Value, right: &Value) -> bool {
    match (left.as_number(), right.as_number()) {
        (Some(left), Some(right)) => left == right,
        _ => matches!((left.as_str(), right.as_str()), (Some(left), Some(right)) if left == right),
    }
}

fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| format_value(value))
}

impl LikertScaleRenderer {
    const NAME: &str = "likert-scale";
}

impl DioxusPatternRenderer for LikertScaleRenderer {
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
        let options: Vec<(Value, String)> = if props.get("options").is_some() {
            get_array(props, "options")
                .iter()
                .map(|option| {
                    (
                        option.get("value").cloned().unwrap_or(Value::Null),
                        get_string_or(option, "label", ""),
                    )
                })
                .collect()
        } else {
            DEFAULT_OPTIONS
                .iter()
                .map(|(value, label)| (Value::Int(*value), (*label).to_string()))
                .collect()
        };
        let current = props.get("value").filter(|value| !value.is_null()).cloned();
        let disabled = get_bool_or(props, "disabled", false);
        let size = get_string_or(props, "size", "md");
        let buttons = get_string_or(props, "variant", "radios") == "buttons";
        let (label_size, gap) = match size.as_str() {
            "sm" => ("text-xs", "gap-1.5"),
            "lg" => ("text-base", "gap-2.5"),
            _ => ("text-sm", "gap-2"),
        };
        let button_size = match size.as_str() {
            "sm" => "px-2 py-1 text-xs",
            "lg" => "px-4 py-2 text-base",
            _ => "px-3 py-1.5 text-sm",
        };
        let question_size = match size.as_str() {
            "sm" => "text-sm",
            "lg" => "text-lg",
            _ => "text-base",
        };
        let question = get_string(props, "question").filter(|question| !question.is_empty());
        let aria_label = get_string(props, "aria-label")
            .or_else(|| question.clone())
            .unwrap_or_else(|| "Likert scale".to_string());
        let class = merge_class_name(
            &format!(
                "{BOX_ATOM_BASE} w-full {}",
                if disabled {
                    "opacity-50 cursor-not-allowed"
                } else {
                    ""
                }
            ),
            props,
        );
        let event = get_string(props, "changeEvent");
        let sink = view.sink;
        let values: Vec<Value> = options.iter().map(|(value, _)| value.clone()).collect();
        let relay_values = values.clone();
        let relay_event = event.clone();
        let relay: EventSink = Callback::new(move |(_, payload): (String, Value)| {
            if disabled {
                return;
            }
            let Some(index) = payload.get("index").and_then(Value::as_int) else {
                return;
            };
            let Some(value) = usize::try_from(index)
                .ok()
                .and_then(|index| relay_values.get(index))
            else {
                return;
            };
            if let (Some(sink), Some(event)) = (sink, &relay_event) {
                sink.call((event.clone(), object_value([("value", value.clone())])));
            }
        });
        let button_view = view.for_source(view.source_trait.clone(), relay);
        let group_id = get_string_or(props, "id", "likert");
        let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
            .into_iter()
            .filter(|attr| !matches!(attr.name, "role" | "aria-label"))
            .collect();
        rsx! {
            div { class: "{class}", role: "radiogroup", "aria-label": "{aria_label}", ..attrs,
                if let Some(question) = question {
                    div { class: "{BOX_ATOM_BASE} mb-3 font-medium text-foreground {question_size}", "{question}" }
                }
                if buttons {
                    div { class: "{BOX_ATOM_BASE} inline-flex w-full items-stretch rounded-interactive overflow-hidden border-[length:var(--border-width)] border-border bg-surface",
                        for (idx, (value, label)) in options.iter().enumerate() {
                            {
                                let selected = current.as_ref().is_some_and(|current| same(current, value));
                                let divider = if idx > 0 { "border-l-[length:var(--border-width)] border-border" } else { "" };
                                let tone = if selected { "bg-primary text-primary-foreground" } else { "bg-surface text-foreground hover:bg-muted" };
                                let spec = pattern_spec("button", [
                                    ("label", Value::string(label.clone())),
                                    ("role", Value::string("radio")),
                                    ("disabled", Value::Bool(disabled)),
                                    ("action", Value::string("LIKERT_SELECT")),
                                    ("actionPayload", object_value([("index", Value::Int(idx as i64))])),
                                    ("className", Value::string(format!("flex-1 text-center font-medium transition-colors duration-instant focus:outline-none focus:ring-[length:var(--focus-ring-width)] focus:ring-ring focus:ring-inset disabled:cursor-not-allowed rounded-none gap-0 shadow-none border-none {divider} {button_size} {tone}"))),
                                ]);
                                rsx! { {registry.render_spec(&spec, None, &button_view)} }
                            }
                        }
                    }
                } else {
                    div { class: "{BOX_ATOM_BASE} flex w-full items-start justify-between {gap}",
                        for (idx, (value, label)) in options.iter().enumerate() {
                            {
                                let selected = current.as_ref().is_some_and(|current| same(current, value));
                                let input_id = format!("{group_id}-{}", display(value));
                                let tone = if selected { "text-foreground font-medium" } else { "text-muted-foreground" };
                                let cursor = if disabled { "cursor-not-allowed" } else { "cursor-pointer" };
                                let radio = pattern_spec("radio", [
                                    ("id", Value::string(input_id.clone())),
                                    ("name", Value::string(group_id.clone())),
                                    ("size", Value::string(size.clone())),
                                    ("checked", Value::Bool(selected)),
                                    ("disabled", Value::Bool(disabled)),
                                ]);
                                let text = pattern_spec("label", [
                                    ("text", Value::string(label.clone())),
                                    ("htmlFor", Value::string(input_id)),
                                    ("className", Value::string(format!("text-center select-none {label_size} {tone} {cursor}"))),
                                ]);
                                let pick = relay;
                                rsx! {
                                    div { key: "{idx}", class: "{BOX_ATOM_BASE} flex flex-1 flex-col items-center {gap}",
                                        onclick: move |_| pick.call((String::new(), object_value([("index", Value::Int(idx as i64))]))),
                                        {registry.render_spec(&radio, None, view)}
                                        {registry.render_spec(&text, None, view)}
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
