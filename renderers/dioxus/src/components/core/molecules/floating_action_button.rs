use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_string, get_string_or};
use orbital_core::Value;

/// Dioxus renderer for the `floating-action-button` pattern.
pub struct FloatingActionButtonRenderer;

#[derive(Props, Clone, PartialEq)]
struct FabProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn position_class(position: &str) -> &'static str {
    match position {
        "bottom-right" => "bottom-6 right-6",
        "bottom-left" => "bottom-6 left-6",
        "bottom-center" => "bottom-6 left-1/2 -translate-x-1/2",
        "top-right" => "top-6 right-6",
        "top-left" => "top-6 left-6",
        "top-center" => "top-6 left-1/2 -translate-x-1/2",
        _ => "",
    }
}

fn sr_only(text: &str) -> Value {
    Value::Array(
        vec![pattern_spec(
            "typography",
            [
                ("as", Value::string("span")),
                ("content", Value::string(text)),
                ("className", Value::string("sr-only")),
            ],
        )]
        .into(),
    )
}

const BOX_BASE: &str = "bg-transparent rounded-none shadow-none";

fn fab_button(
    variant: &str,
    icon: Value,
    aria_label: &str,
    extra: &str,
    more: Vec<(&str, Value)>,
) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("size", Value::string("lg")),
        ("icon", icon),
        (
            "className",
            Value::String(format!("rounded-full shadow-elevation-popover{extra}").into()),
        ),
        ("aria-label", Value::string(aria_label)),
    ];
    fields.extend(more);
    pattern_spec("button", fields)
}

#[allow(non_snake_case)]
fn Fab(input: FabProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let fields = props.as_object();
    let position = get_string_or(props, "position", "bottom-right");
    let sink = view.sink;
    let actions: Vec<Value> = fields
        .and_then(|fields| fields.get("actions"))
        .and_then(Value::as_array)
        .map(|items| items.iter().cloned().collect())
        .unwrap_or_default();
    let expanded = use_signal(|| false);
    let left = position.contains("left");
    let pos = position_class(&position);

    if actions.is_empty() {
        let Some(icon) = fields
            .and_then(|fields| fields.get("icon"))
            .filter(|icon| !icon.is_null())
        else {
            return rsx! {};
        };
        let label = get_string(props, "label").filter(|label| !label.is_empty());
        let aria = get_string(props, "aria-label")
            .or(label)
            .unwrap_or_else(|| crate::i18n::t("fab.action"));
        let mut spec = vec![
            (
                "variant",
                Value::String(get_string_or(props, "variant", "primary").into()),
            ),
            ("size", Value::string("lg")),
            ("icon", icon.clone()),
            (
                "className",
                Value::string("rounded-full shadow-elevation-popover"),
            ),
            ("aria-label", Value::String(aria.into())),
        ];
        if let Some(label) = get_string(props, "label").filter(|label| !label.is_empty()) {
            spec.push(("children", sr_only(&label)));
        }
        if let Some(action) = get_string(props, "action") {
            spec.push(("action", Value::String(action.into())));
            if let Some(payload) = fields.and_then(|fields| fields.get("actionPayload")) {
                spec.push(("actionPayload", payload.clone()));
            }
        }
        let spec = pattern_spec("button", spec);
        let class = merge_class_name(&format!("{BOX_BASE} fixed z-50 flex {pos}"), props);
        return rsx! { div { class: "{class}", {registry.render_spec(&spec, None, &view)} } };
    }

    let is_expanded = expanded();
    let class = merge_class_name(
        &format!(
            "{BOX_BASE} fixed z-50 flex flex-col items-end gap-3 {pos} {}",
            if left { "items-start" } else { "" }
        ),
        props,
    );
    let main_aria = get_string(props, "aria-label").unwrap_or_else(|| {
        if is_expanded {
            crate::i18n::t("fab.closeActions")
        } else {
            crate::i18n::t("fab.openActions")
        }
        .to_string()
    });
    let main = fab_button(
        if is_expanded { "secondary" } else { "primary" },
        Value::string(if is_expanded { "x" } else { "plus" }),
        &main_aria,
        " transition-all duration-normal",
        vec![
            ("aria-expanded", Value::Bool(is_expanded)),
            (
                "children",
                sr_only(&if is_expanded {
                    crate::i18n::t("common.close")
                } else {
                    crate::i18n::t("common.open")
                }),
            ),
            ("action", Value::string("__local_fab_main")),
        ],
    );
    let single_event = (actions.len() == 1)
        .then(|| get_string(&actions[0], "event"))
        .flatten();
    let single_id = get_string_or(&actions[0], "id", "");
    let multiple = actions.len() > 1;
    let main_expanded = expanded;
    let main_node =
        crate::renderer::render_with_local_sink(&view, registry, &main, move |name, _| {
            if name != "__local_fab_main" {
                return false;
            }
            if multiple {
                let mut state = main_expanded;
                let next = !state();
                state.set(next);
            } else if let (Some(sink), Some(event)) = (sink, &single_event) {
                sink.call((
                    event.clone(),
                    object_value([("actionId", Value::String(single_id.clone().into()))]),
                ));
            }
            true
        });
    let label_class = "text-foreground bg-card px-2 py-1 rounded-interactive shadow-elevation-card whitespace-nowrap";
    let list_align = if left { "items-start" } else { "items-end" };

    rsx! {
        div { class: "{class}",
            if is_expanded && multiple {
                div { class: "flex flex-col gap-3 {list_align}",
                    for (index, action) in actions.iter().enumerate() {
                        {
                            let id = get_string_or(action, "id", "");
                            let label = get_string_or(action, "label", "");
                            let event = get_string(action, "event");
                            let button = fab_button(
                                &get_string_or(action, "variant", "primary"),
                                action.get("icon").cloned().unwrap_or(Value::Null),
                                &label,
                                "",
                                vec![
                                    ("action", Value::string("__local_fab_action")),
                                    ("children", sr_only(&label)),
                                ],
                            );
                            let item_expanded = expanded;
                            let item_id = id.clone();
                            let button_node = crate::renderer::render_with_local_sink(&view, registry, &button, move |name, _| {
                                if name != "__local_fab_action" {
                                    return false;
                                }
                                let mut state = item_expanded;
                                state.set(false);
                                if let (Some(sink), Some(event)) = (sink, &event) {
                                    sink.call((event.clone(), object_value([("actionId", Value::String(item_id.clone().into()))])));
                                }
                                true
                            });
                            let label_spec = pattern_spec("typography", [
                                ("variant", Value::string("small")),
                                ("className", Value::string(label_class)),
                                ("content", Value::String(label.into())),
                            ]);
                            rsx! {
                                div {
                                    key: "{id}",
                                    class: "flex flex-row gap-2 items-center transition-all duration-fast",
                                    style: "opacity:1;transform:translateY(0);transition-delay:{index * 50}ms",
                                    if !left { {registry.render_spec(&label_spec, None, &view)} }
                                    {button_node}
                                    if left { {registry.render_spec(&label_spec, None, &view)} }
                                }
                            }
                        }
                    }
                }
            }
            {main_node}
        }
    }
}

impl DioxusPatternRenderer for FloatingActionButtonRenderer {
    fn pattern_name(&self) -> &str {
        "floating-action-button"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Fab { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
