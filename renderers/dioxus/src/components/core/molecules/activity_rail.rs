//! Native Dioxus renderer for the `activity-rail` pattern: the web `ActivityRail`.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, merge_class_name, object_value, pattern_spec};
use crate::renderer::{DioxusPatternRenderer, DioxusView, DockCommand, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_array, get_bool_or, get_string, get_string_or};
use orbital_core::Value;

const PICK_EVENT: &str = "__activity_rail_pick";

pub struct ActivityRailRenderer;

impl DioxusPatternRenderer for ActivityRailRenderer {
    fn pattern_name(&self) -> &str {
        "activity-rail"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let items = get_array(props, "items");
        let active = get_string_or(props, "active", "");
        let on_change = get_string(props, "onChange").filter(|event| !event.is_empty());
        let test_id = get_string(props, "testId");
        let footer = props
            .as_object()
            .and_then(|fields| fields.get("footer"))
            .cloned();
        let dock = view.dock.clone();
        let compact = dock.as_ref().is_some_and(|context| context.compact);
        let sink = view.sink;

        let entries: Vec<(Value, DioxusView<'_>)> = items
            .iter()
            .map(|item| {
                let id = get_string_or(item, "id", "");
                let label = get_string_or(item, "label", "");
                let icon = get_string_or(item, "icon", "");
                let selected = active == id;
                let opens_panel = get_bool_or(item, "opensPanel", true);
                let variant = if selected { "primary" } else { "ghost" };
                let action = on_change.clone().unwrap_or_else(|| PICK_EVENT.to_string());
                let forward = on_change.clone();
                let item_dock = dock.clone();
                let relay: EventSink = Callback::new(move |(event, payload): (String, Value)| {
                    if let (Some(sink), Some(_)) = (sink, &forward) {
                        sink.call((event, payload));
                    }
                    if let Some(context) = &item_dock {
                        if context.compact {
                            context.command.call(if opens_panel {
                                DockCommand::ShowPanel
                            } else {
                                DockCommand::CloseMenu
                            });
                        }
                    }
                });
                let payload = object_value([("id", Value::string(id.clone()))]);
                let mut fields = vec![
                    ("variant", Value::string(variant)),
                    ("aria-pressed", Value::Bool(selected)),
                    ("action", Value::string(action)),
                    ("actionPayload", payload),
                ];
                let spec = if compact {
                    fields.push(("label", Value::string(label)));
                    fields.push(("leftIcon", Value::string(icon)));
                    fields.push((
                        "className",
                        Value::string("w-full h-12 justify-start gap-3"),
                    ));
                    pattern_spec("button", fields)
                } else {
                    fields.push(("label", Value::string(label)));
                    fields.push(("icon", Value::string(icon)));
                    fields.push(("className", Value::string("w-9 h-9")));
                    pattern_spec("icon-button", fields)
                };
                (spec, view.for_source(view.source_trait.clone(), relay))
            })
            .collect();

        let column = cn(&[
            "flex flex-col gap-1 items-stretch justify-start",
            "items-center",
        ]);
        let class = if compact {
            merge_class_name(
                &cn(&[
                    "flex flex-col gap-1 items-stretch justify-start",
                    "w-full p-2",
                ]),
                props,
            )
        } else {
            merge_class_name(
                &cn(&[
                    "bg-card rounded-none shadow-none",
                    "w-12 h-full flex flex-col items-center justify-between py-2",
                ]),
                props,
            )
        };
        rsx! {
            div { class: "contents", ..shared_root_attrs(props),
                if compact {
                    div { class: "{class}", "data-testid": test_id,
                        for (spec, item_view) in entries.iter() {
                            {registry.render_spec(spec, None, item_view)}
                        }
                        if let Some(footer) = &footer { {view.render_node(registry, footer)} }
                    }
                } else {
                    div { class: "{class}", "data-testid": test_id,
                        div { class: "{column}",
                            for (spec, item_view) in entries.iter() {
                                {registry.render_spec(spec, None, item_view)}
                            }
                        }
                        if let Some(footer) = &footer { {view.render_node(registry, footer)} }
                    }
                }
            }
        }
    }
}
