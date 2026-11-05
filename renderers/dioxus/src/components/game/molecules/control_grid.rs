use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_string, get_string_or, merge_class_name,
};
use orbital_core::{Value, ValueMap};

pub struct ControlGridRenderer;

fn event_payload(key: &str, id: &str, pressed: bool) -> Value {
    let mut payload = ValueMap::new();
    payload.insert(key.into(), Value::String(id.to_string().into()));
    payload.insert("pressed".into(), Value::Bool(pressed));
    Value::Object(payload.into())
}

struct Control<'a> {
    id: String,
    label: String,
    icon: Option<String>,
    asset: Option<Value>,
    variant: String,
    direction: bool,
    size: &'a str,
    disabled: bool,
}

fn render_control(
    control: Control<'_>,
    props: &Value,
    entity: Option<&Value>,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    let mut spec = ValueMap::new();
    spec.insert("type".into(), Value::String("control-button".into()));
    spec.insert("label".into(), Value::String(control.label.clone().into()));
    spec.insert(
        "size".into(),
        Value::String(control.size.to_string().into()),
    );
    spec.insert("variant".into(), Value::String(control.variant.into()));
    spec.insert("disabled".into(), Value::Bool(control.disabled));
    spec.insert("pressEvent".into(), Value::String("__grid_press".into()));
    spec.insert(
        "releaseEvent".into(),
        Value::String("__grid_release".into()),
    );
    if let Some(icon) = control.icon {
        spec.insert("icon".into(), Value::String(icon.into()));
    }
    if let Some(asset) = control.asset {
        spec.insert("assetUrl".into(), asset);
    }
    let action_event = get_string(props, "actionEvent");
    let on_action = get_string(props, "onAction");
    let direction_event = get_string(props, "directionEvent");
    let on_direction = get_string(props, "onDirection");
    let direction_events = props
        .as_object()
        .and_then(|value| value.get("directionEvents"))
        .cloned();
    let release_events = props
        .as_object()
        .and_then(|value| value.get("directionReleaseEvents"))
        .cloned();
    let id = control.id;
    let is_direction = control.direction;
    let sink = view.sink;
    let relay: EventSink = Callback::new(move |(name, _): (String, Value)| {
        let pressed = match name.as_str() {
            "__grid_press" => true,
            "__grid_release" => false,
            _ => return,
        };
        let Some(sink) = sink else { return };
        if let Some(event) = &action_event {
            sink.call((event.clone(), event_payload("id", &id, pressed)));
        }
        if let Some(event) = &on_action {
            sink.call((event.clone(), event_payload("id", &id, pressed)));
        }
        if is_direction {
            let semantic = if pressed {
                direction_events.as_ref()
            } else {
                release_events.as_ref()
            };
            if let Some(event) = semantic.and_then(|map| get_string(map, &id)) {
                sink.call((event, Value::Null));
            }
            if let Some(event) = &direction_event {
                sink.call((event.clone(), event_payload("direction", &id, pressed)));
            }
            if let Some(event) = &on_direction {
                sink.call((event.clone(), event_payload("direction", &id, pressed)));
            }
        }
    });
    let child_view = view.for_source(view.source_trait.clone(), relay);
    registry.render_spec(&Value::Object(spec.into()), entity, &child_view)
}

impl DioxusPatternRenderer for ControlGridRenderer {
    fn pattern_name(&self) -> &str {
        "control-grid"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let visibility = get_string_or(props, "visibility", "auto");
        if visibility == "auto" && !cfg!(any(target_os = "android", target_os = "ios")) {
            return rsx! {};
        }
        let class = merge_class_name("almadar-control-grid", props);
        let root_attrs = shared_root_attrs(props);
        let size = get_string_or(props, "size", "md");
        let disabled = get_bool_or(props, "disabled", false);
        let kind = get_string_or(props, "kind", "dpad");
        if kind == "actions" {
            let buttons = get_array(props, "buttons");
            let layout = get_string_or(props, "layout", "horizontal");
            let layout_style = match layout.as_str() {
                "vertical" => "display:flex;flex-direction:column;gap:8px",
                "diamond" => "display:grid;grid-template-columns:repeat(3,max-content);gap:8px",
                _ => "display:flex;flex-direction:row;gap:8px",
            };
            return rsx! {
                div { class: "{class}", style: "{layout_style}", ..root_attrs,
                    for (index, button) in buttons.iter().enumerate() {
                        {
                            let id = get_string_or(button, "id", "");
                            let label = get_string_or(button, "label", &id);
                            let icon = get_string(button, "icon");
                            let asset = button.as_object().and_then(|value| value.get("assetUrl")).cloned();
                            let variant = get_string_or(button, "variant", "secondary");
                            let grid_cell = match (layout.as_str(), buttons.len(), index) {
                                ("diamond", 4, 0) => "grid-column:2;grid-row:1",
                                ("diamond", 4, 1) => "grid-column:3;grid-row:2",
                                ("diamond", 4, 2) => "grid-column:2;grid-row:3",
                                ("diamond", 4, 3) => "grid-column:1;grid-row:2",
                                _ => "",
                            };
                            rsx! {
                                div { key: "{id}", style: "{grid_cell}",
                                    {render_control(Control { id, label, icon, asset, variant, direction: false, size: &size, disabled }, props, entity, view, registry)}
                                }
                            }
                        }
                    }
                }
            };
        }

        let include_diagonals = get_bool_or(props, "includeDiagonals", false);
        let semantic = props
            .as_object()
            .and_then(|value| value.get("directionEvents"));
        let directions = [
            ("up-left", "◤"),
            ("up", "▲"),
            ("up-right", "◥"),
            ("left", "◀"),
            ("", ""),
            ("right", "▶"),
            ("down-left", "◣"),
            ("down", "▼"),
            ("down-right", "◢"),
        ];
        let assets = props
            .as_object()
            .and_then(|value| value.get("directionAssets"));
        rsx! {
            div { class: "{class}", style: "display:grid;grid-template-columns:repeat(3,max-content);gap:4px", ..root_attrs,
                for (direction, glyph) in directions {
                    if direction.is_empty()
                        || (direction.contains('-') && !include_diagonals)
                        || (semantic.is_some() && semantic.and_then(|map| get_string(map, direction)).is_none()) {
                        span { class: "almadar-control-grid__empty" }
                    } else {
                        {
                            let asset = assets.and_then(|map| map.as_object()).and_then(|map| map.get(direction)).cloned();
                            render_control(Control {
                                id: direction.to_string(),
                                label: direction.to_string(),
                                icon: Some(glyph.to_string()),
                                asset,
                                variant: "secondary".into(),
                                direction: true,
                                size: &size,
                                disabled,
                            }, props, entity, view, registry)
                        }
                    }
                }
            }
        }
    }
}
