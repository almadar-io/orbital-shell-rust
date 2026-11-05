use crate::registry::DioxusPatternRegistry;
use crate::renderer::DockCommand;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_string, get_string_or, icon_name_to_glyph, merge_class_name,
};
use orbital_core::Value;

pub struct ActivityRailRenderer;

impl DioxusPatternRenderer for ActivityRailRenderer {
    fn pattern_name(&self) -> &str {
        "activity-rail"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("almadar-activity-rail", props);
        let items = get_array(props, "items");
        let active = get_string_or(props, "active", "");
        let on_change = get_string(props, "onChange");
        let on_select = get_string(props, "onSelect");
        let test_id = get_string(props, "testId");
        let footer = props
            .as_object()
            .and_then(|value| value.get("footer"))
            .cloned();
        let sink = view.sink;
        let dock = view.dock.clone();
        let compact = dock.as_ref().is_some_and(|context| context.compact);
        let style = if compact {
            "display:flex;flex-direction:column;width:100%;padding:8px"
        } else {
            "display:flex;flex-direction:column;align-items:center;width:48px;height:100%;padding:8px 0"
        };
        rsx! {
            nav { class: "{class}", style: "{style}", "data-testid": test_id,
                for item in items.iter() {
                    {
                        let id = get_string_or(item, "id", "");
                        let label = get_string_or(item, "label", "");
                        let icon = get_string_or(item, "icon", "");
                        let glyph = icon_name_to_glyph(&icon);
                        let selected = active == id;
                        let change = on_change.clone();
                        let select = on_select.clone();
                        let item_dock = dock.clone();
                        let opens_panel = get_bool_or(item, "opensPanel", true);
                        let button_style = if compact { "display:flex;align-items:center;justify-content:flex-start;gap:12px;width:100%;min-height:48px" } else { "display:flex;align-items:center;justify-content:center;width:36px;height:36px" };
                        let label_style = if compact { "display:inline" } else { "display:none" };
                        rsx! {
                            button {
                                key: "{id}",
                                class: "almadar-button almadar-activity-rail__item",
                                style: "{button_style}",
                                title: "{label}",
                                "aria-label": "{label}",
                                "aria-pressed": selected,
                                onclick: move |_| {
                                    if let Some(sink) = sink {
                                        let mut payload = orbital_core::ValueMap::new();
                                        payload.insert("id".into(), Value::String(id.clone().into()));
                                        if let Some(event) = &change {
                                            sink.call((event.clone(), Value::Object(payload.clone().into())));
                                        }
                                        if let Some(event) = &select {
                                            sink.call((event.clone(), Value::Object(payload.into())));
                                        }
                                    }
                                    if let Some(context) = &item_dock {
                                        if context.compact {
                                            context.command.call(if opens_panel { DockCommand::ShowPanel } else { DockCommand::CloseMenu });
                                        }
                                    }
                                },
                                span { class: "almadar-activity-rail__icon", "{glyph}" }
                                span { class: "almadar-activity-rail__label", style: "{label_style}", "{label}" }
                            }
                        }
                    }
                }
                if let Some(footer) = &footer {
                    div { class: "almadar-activity-rail__footer", {registry.render_spec(footer, entity, view)} }
                }
            }
        }
    }
}
