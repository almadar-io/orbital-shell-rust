//! Native Dioxus body for the `matrix-question` pattern, mirroring `MatrixQuestion.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct MatrixQuestionRenderer;

const DEFAULT_COLUMNS: [(i64, &str); 5] = [
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

impl MatrixQuestionRenderer {
    const NAME: &str = "matrix-question";
}

impl DioxusPatternRenderer for MatrixQuestionRenderer {
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
        let small = get_string_or(props, "size", "md") == "sm";
        let (cell, radio_size, label_size) = if small {
            ("px-2 py-1.5 text-xs", "sm", "text-xs")
        } else {
            ("px-3 py-2 text-sm", "md", "text-sm")
        };
        let columns: Vec<(Value, String)> = if props.get("columns").is_some() {
            get_array(props, "columns")
                .iter()
                .map(|column| {
                    (
                        column.get("value").cloned().unwrap_or(Value::Null),
                        get_string_or(column, "label", ""),
                    )
                })
                .collect()
        } else {
            DEFAULT_COLUMNS
                .iter()
                .map(|(value, label)| (Value::Int(*value), (*label).to_string()))
                .collect()
        };
        let rows = get_array(props, "rows");
        let values = props.get("values");
        let disabled = get_bool_or(props, "disabled", false);
        let class = merge_class_name(&format!("{BOX_ATOM_BASE} w-full"), props);
        let title = get_string(props, "title").filter(|title| !title.is_empty());
        let title_spec = title.map(|title| {
            pattern_spec(
                "typography",
                [
                    ("variant", Value::string("h4")),
                    ("className", Value::string("mb-3")),
                    ("content", Value::string(title)),
                ],
            )
        });
        let event = get_string(props, "changeEvent");
        let sink = view.sink;
        let attrs = crate::registry::shared_root_attrs(props);
        let row_values: Vec<Value> = columns.iter().map(|(value, _)| value.clone()).collect();
        rsx! {
            div { class: "{class}", ..attrs,
                if let Some(spec) = title_spec.as_ref() { {registry.render_spec(spec, None, view)} }
                div { class: "{BOX_ATOM_BASE} overflow-auto overflow-x-auto",
                    table { class: "{BOX_ATOM_BASE} w-full border-collapse",
                        thead { class: "{BOX_ATOM_BASE}",
                            tr { class: "{BOX_ATOM_BASE} border-b border-border",
                                th { role: "columnheader", class: "{BOX_ATOM_BASE} text-left font-medium text-muted-foreground {cell}" }
                                for (value, label) in columns.iter() {
                                    th { key: "{display(value)}", role: "columnheader", class: "{BOX_ATOM_BASE} text-center font-medium text-muted-foreground {cell}", "{label}" }
                                }
                            }
                        }
                        tbody { class: "{BOX_ATOM_BASE}",
                            for (idx, row) in rows.iter().enumerate() {
                                {
                                    let row_id = row.get("id").map(display).unwrap_or_default();
                                    let row_label = get_string_or(row, "label", "");
                                    let group = format!("matrix-{row_id}");
                                    let selected = values.and_then(|values| values.get(&row_id)).cloned();
                                    let stripe = if idx % 2 == 1 { "bg-muted/30" } else { "" };
                                    let dim = if disabled { "opacity-60" } else { "" };
                                    rsx! {
                                        tr { key: "{row_id}", class: "{BOX_ATOM_BASE} border-b border-border {stripe}",
                                            th { role: "rowheader", class: "{BOX_ATOM_BASE} text-left font-normal text-foreground {cell} {label_size}", "{row_label}" }
                                            for (col_idx, (value, label)) in columns.iter().enumerate() {
                                                {
                                                    let checked = selected.as_ref().is_some_and(|selected| same(selected, value));
                                                    let radio = pattern_spec("radio", [
                                                        ("id", Value::string(format!("{group}-{}", display(value)))),
                                                        ("name", Value::string(group.clone())),
                                                        ("value", Value::string(display(value))),
                                                        ("checked", Value::Bool(checked)),
                                                        ("disabled", Value::Bool(disabled)),
                                                        ("size", Value::string(radio_size)),
                                                        ("aria-label", Value::string(format!("{row_label}: {label}"))),
                                                    ]);
                                                    let pick_event = event.clone();
                                                    let pick_row = row_id.clone();
                                                    let pick_value = row_values.get(col_idx).cloned().unwrap_or(Value::Null);
                                                    rsx! {
                                                        td { key: "{display(value)}", class: "{BOX_ATOM_BASE} text-center {cell}",
                                                            div { class: "{BOX_ATOM_BASE} inline-flex items-center justify-center {dim}",
                                                                onclick: move |_| {
                                                                    if disabled { return; }
                                                                    if let (Some(sink), Some(event)) = (sink, &pick_event) {
                                                                        sink.call((event.clone(), object_value([("rowId", Value::string(pick_row.clone())), ("value", pick_value.clone())])));
                                                                    }
                                                                },
                                                                {registry.render_spec(&radio, None, view)}
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
                    }
                }
            }
        }
    }
}
