use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_number_or, get_string, get_string_or, row_action_is_shown,
};
use orbital_core::client::recurse::child_patterns;
use orbital_core::{Value, ValueMap};

const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct SwipeableRowRenderer;

fn action_button(action: &Value, item: Option<&Value>) -> Option<Value> {
    let event = get_string(action, "event")?;
    let mut payload = ValueMap::new();
    if let Some(Value::Object(fields)) = action.get("eventPayload") {
        for (key, value) in fields.iter() {
            payload.insert(key.clone(), value.clone());
        }
    }
    if let Some(row) = item {
        payload.insert("row".into(), row.clone());
    }
    let mut fields = vec![
        ("label", Value::string(get_string_or(action, "label", ""))),
        (
            "variant",
            Value::string(get_string_or(action, "variant", "secondary")),
        ),
        ("size", Value::string("sm")),
        ("className", Value::string("flex-1 rounded-none h-full")),
        ("action", Value::string(event)),
        ("actionPayload", Value::Object(payload.into())),
    ];
    if let Some(icon) = action.get("icon") {
        fields.push(("icon", icon.clone()));
    }
    Some(pattern_spec("button", fields))
}

fn shown_actions(props: &Value, key: &str, item: &Value) -> Vec<Value> {
    get_array(props, key)
        .into_iter()
        .filter(|action| row_action_is_shown(action, item, None))
        .collect()
}

impl DioxusPatternRenderer for SwipeableRowRenderer {
    fn pattern_name(&self) -> &str {
        "swipeable-row"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let threshold = get_number_or(props, "threshold", 80.0);
        let item = props.get("itemData");
        let row = item.cloned().unwrap_or_else(|| object_value::<&str>([]));
        let root = cn(&[
            BOX,
            "overflow-hidden relative",
            &cn(&[
                "touch-pan-y",
                get_string_or(props, "className", "").as_str(),
            ]),
        ]);
        let content = cn(&[BOX, "relative bg-surface select-none"]);
        let sides = [
            ("left-0", shown_actions(props, "leftActions", &row)),
            ("right-0", shown_actions(props, "rightActions", &row)),
        ];
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{root}", ..shared_root_attrs(props),
                for (side, actions) in sides.iter() {
                    if !actions.is_empty() {
                        div {
                            class: "flex flex-row gap-1 items-stretch justify-start absolute top-0 bottom-0 {side}",
                            style: "width:{threshold}px",
                            for action in actions.iter() {
                                if let Some(button) = action_button(action, item) {
                                    {registry.render_spec(&button, None, view)}
                                }
                            }
                        }
                    }
                }
                div { class: "{content}", style: "transform:translateX(0px);transition:transform 200ms ease-out",
                    for (child, source) in children.iter() {
                        {view.render_child(registry, child, source.as_deref())}
                    }
                }
            }
        }
    }
}
