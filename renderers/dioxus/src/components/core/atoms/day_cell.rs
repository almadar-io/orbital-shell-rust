use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::calendar::calendar_day;
use orbital_core::client::helpers::{get_bool_or, get_string, merge_class_name};
use orbital_core::Value;

pub struct DayCellRenderer;

impl DioxusPatternRenderer for DayCellRenderer {
    fn pattern_name(&self) -> &str {
        "day-cell"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let day = calendar_day(get_string(props, "date").as_deref());
        let is_today = get_bool_or(props, "isToday", false);
        let on_click = get_string(props, "onClick");
        let root_class = merge_class_name(
            if is_today {
                "p-2 text-center cursor-pointer hover:bg-muted transition-colors bg-primary/10"
            } else {
                "p-2 text-center cursor-pointer hover:bg-muted transition-colors"
            },
            props,
        );
        let weekday_class = if is_today {
            "text-sm font-medium text-primary"
        } else {
            "text-sm font-medium text-muted-foreground"
        };
        let number_class = if is_today {
            "flex rounded-full h-8 w-8 mx-auto items-center justify-center bg-primary text-primary-foreground"
        } else {
            "flex rounded-full h-8 w-8 mx-auto items-center justify-center"
        };
        let aria_label = get_string(props, "aria-label").unwrap_or_else(|| day.label.clone());
        let weekday = day.weekday;
        let number = day.day_number;

        if let Some(event) = on_click {
            let sink = view.sink;
            let date = day.iso;
            rsx! {
                button {
                    class: "{root_class}",
                    aria_label: "{aria_label}",
                    onclick: move |_| {
                        if let Some(sink) = sink {
                            sink.call((event.clone(), Value::string(date.clone())));
                        }
                    },
                    span { class: "{weekday_class}", "{weekday}" }
                    span {
                        class: "{number_class}",
                        span { class: "font-semibold", "{number}" }
                    }
                }
            }
        } else {
            rsx! {
                div {
                    class: "{root_class}",
                    span { class: "{weekday_class}", "{weekday}" }
                    span {
                        class: "{number_class}",
                        span { class: "font-semibold", "{number}" }
                    }
                }
            }
        }
    }
}
