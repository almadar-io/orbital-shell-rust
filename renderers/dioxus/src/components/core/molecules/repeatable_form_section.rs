use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const ADD: &str = "repeatable:add";
const REMOVE: &str = "repeatable:remove";
const MOVE: &str = "repeatable:move";

/// Dioxus renderer for the `repeatable-form-section` pattern.
pub struct RepeatableFormSectionRenderer;

impl RepeatableFormSectionRenderer {
    const NAME: &str = "repeatable-form-section";
}

impl DioxusPatternRenderer for RepeatableFormSectionRenderer {
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
        rsx! { RepeatableBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct RepeatableBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn text(value: impl AsRef<str>) -> Value {
    Value::string(value.as_ref().to_string())
}

fn typography(variant: &str, color: Option<&str>, weight: Option<&str>, content: &str) -> Value {
    let mut fields = vec![("variant", text(variant)), ("content", text(content))];
    if let Some(color) = color {
        fields.push(("color", text(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", text(weight)));
    }
    pattern_spec("typography", fields)
}

fn button(
    variant: &str,
    label: &str,
    icon: &str,
    action: &str,
    payload: Value,
    extra: Vec<(&str, Value)>,
) -> Value {
    let mut fields = vec![
        ("variant", text(variant)),
        ("size", text("sm")),
        ("label", text(label)),
        ("icon", text(icon)),
        ("action", text(action)),
        ("actionPayload", payload),
    ];
    fields.extend(extra);
    pattern_spec("button", fields)
}

fn item_id(item: &Value) -> String {
    match item.get("id") {
        Some(Value::String(id)) => id.to_string(),
        Some(Value::Int(id)) => id.to_string(),
        Some(Value::Number(id)) => id.to_string(),
        _ => String::new(),
    }
}

const STACK_MD: &str = "flex flex-col gap-4 items-stretch justify-start";
const STACK_SM: &str = "flex flex-col gap-2 items-stretch justify-start";
const ROW_BETWEEN: &str = "flex flex-row gap-4 items-center justify-between";
const ROW_SM_CENTER: &str = "flex flex-row gap-2 items-center justify-start";
const CARD: &str = "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md";

#[allow(non_snake_case)]
fn RepeatableBody(body: RepeatableBodyProps) -> Element {
    let props = &body.props;
    let view = body.view.as_view();
    let registry = &body.registry;
    let section_type = get_string_or(props, "sectionType", "");
    let title = get_string_or(props, "title", "");
    let items = get_array(props, "items");
    let min_items = get_number_or(props, "minItems", 0.0) as usize;
    let max_items = get_number(props, "maxItems").map(|max| max as usize);
    let allow_reorder = get_bool_or(props, "allowReorder", false);
    let add_label = get_string_or(props, "addLabel", &crate::i18n::t("common.add"));
    let empty_message = get_string_or(props, "emptyMessage", &crate::i18n::t("empty.noItemsAdded"));
    let read_only = get_bool_or(props, "readOnly", false);
    let track_state = get_bool_or(props, "trackAddedInState", false);
    let current_state = get_string(props, "currentState").filter(|state| !state.is_empty());
    let show_audit = get_bool_or(props, "showAuditInfo", false);
    let on_add = get_string(props, "onAdd").filter(|event| !event.is_empty());
    let on_remove = get_string(props, "onRemove").filter(|event| !event.is_empty());
    let on_reorder = get_string(props, "onReorder").filter(|event| !event.is_empty());

    let count = items.len();
    let can_add = !read_only && max_items.is_none_or(|max| count < max);
    let can_remove = !read_only && count > min_items;
    let can_reorder = allow_reorder && !read_only;

    let real_sink = view.sink;
    let wrapped = Callback::new(move |(event, payload): (String, Value)| {
        let Some(sink) = real_sink else { return };
        match event.as_str() {
            ADD => {
                if let Some(extra) = &on_add {
                    sink.call((extra.clone(), Value::Null));
                }
                sink.call(("SECTION_ADDED".to_string(), payload));
            }
            REMOVE => {
                if let Some(extra) = &on_remove {
                    sink.call((extra.clone(), payload.clone()));
                }
                sink.call(("SECTION_REMOVED".to_string(), payload));
            }
            MOVE => {
                if let Some(extra) = &on_reorder {
                    sink.call((extra.clone(), payload));
                }
            }
            _ => {}
        }
    });
    let control_view = view.for_source(view.source_trait.clone(), wrapped);

    let count_label = match max_items {
        Some(max) => format!("({count}/{max})"),
        None => format!("({count})"),
    };
    let mut add_fields = vec![
        ("sectionType", text(&section_type)),
        ("index", Value::Int(count as i64)),
    ];
    if track_state {
        if let Some(state) = &current_state {
            add_fields.push(("addedInState", text(state)));
        }
    }
    let add_payload = object_value(add_fields);
    let header_add = button(
        "secondary",
        &add_label,
        "plus",
        ADD,
        add_payload.clone(),
        vec![],
    );
    let empty_add = button("primary", &add_label, "plus", ADD, add_payload, vec![]);
    let lambda = props.get("renderItem").and_then(render_item_lambda);
    let outer_class = merge_class_name(&format!("{STACK_MD} w-full"), props);

    rsx! {
        div { class: "{outer_class}", style: "height:auto", ..shared_root_attrs(props),
            div { class: "{ROW_BETWEEN}", style: "height:auto",
                div { class: "{ROW_SM_CENTER}", style: "height:auto",
                    {registry.render_spec(&typography("h4", None, None, &title), None, &control_view)}
                    {registry.render_spec(&typography("caption", Some("muted"), None, &count_label), None, &control_view)}
                }
                if can_add { {registry.render_spec(&header_add, None, &control_view)} }
            }
            if count == 0 {
                div { class: "{CARD} p-6", style: "height:auto",
                    div { class: "flex flex-col gap-2 items-center justify-start", style: "height:auto",
                        {registry.render_spec(&typography("body", Some("muted"), None, &empty_message), None, &control_view)}
                        if can_add { {registry.render_spec(&empty_add, None, &control_view)} }
                    }
                }
            } else {
                div { class: "{STACK_SM}", style: "height:auto",
                    for (index, item) in items.iter().enumerate() {
                        {
                            let id = item_id(item);
                            let added_state = get_string(item, "addedInState").filter(|state| !state.is_empty());
                            let added_at = get_string(item, "addedAt").filter(|at| !at.is_empty());
                            let remove_payload = object_value([("sectionType", text(&section_type)), ("index", Value::Int(index as i64)), ("itemId", text(&id))]);
                            let up = object_value([("fromIndex", Value::Int(index as i64)), ("toIndex", Value::Int(index as i64 - 1))]);
                            let down = object_value([("fromIndex", Value::Int(index as i64)), ("toIndex", Value::Int(index as i64 + 1))]);
                            let rendered = lambda.as_ref().map(|lambda| lambda.apply(item, index));
                            rsx! {
                                div { key: "{id}", class: "{CARD} p-4", style: "height:auto",
                                    div { class: "{STACK_SM}", style: "height:auto",
                                        if show_audit && (added_state.is_some() || added_at.is_some()) {
                                            div { class: "{ROW_BETWEEN} pb-2 border-b border-border", style: "height:auto",
                                                div { class: "{ROW_SM_CENTER}", style: "height:auto",
                                                    if let Some(state) = &added_state {
                                                        div { class: "flex flex-row gap-1 items-center justify-start", style: "height:auto",
                                                            {registry.render_spec(&typography("caption", Some("muted"), None, &crate::i18n::t("repeatable.addedIn")), None, &control_view)}
                                                            {registry.render_spec(&typography("caption", None, Some("semibold"), state), None, &control_view)}
                                                        }
                                                    }
                                                }
                                                if let Some(at) = &added_at {
                                                    {registry.render_spec(&typography("caption", Some("muted"), None, at), None, &control_view)}
                                                }
                                            }
                                        }
                                        div { class: "flex flex-row gap-2 items-start justify-start", style: "height:auto",
                                            if can_reorder {
                                                div { class: "flex flex-col gap-0 items-stretch justify-start", style: "height:auto",
                                                    {registry.render_spec(&button("ghost", "", "chevron-up", MOVE, up, vec![("aria-label", text(crate::i18n::t("common.moveUp"))), ("disabled", Value::Bool(index == 0))]), None, &control_view)}
                                                    {registry.render_spec(&button("ghost", "", "chevron-down", MOVE, down, vec![("aria-label", text(crate::i18n::t("common.moveDown"))), ("disabled", Value::Bool(index + 1 == count))]), None, &control_view)}
                                                }
                                            }
                                            div { class: "bg-transparent rounded-none shadow-none flex-1",
                                                if let Some(child) = &rendered { {registry.render_spec(child, None, &view)} }
                                            }
                                            if can_remove {
                                                {registry.render_spec(&button("ghost", "", "trash-2", REMOVE, remove_payload, vec![("aria-label", text(crate::i18n::t("common.remove"))), ("className", text("text-foreground hover:bg-error/10"))]), None, &control_view)}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if count < min_items {
                {registry.render_spec(&typography("caption", Some("warning"), None, &if min_items == 1 { crate::i18n::t("repeatable.minItemsOne") } else { format!("At least {min_items} items required") }), None, &control_view)}
            }
        }
    }
}
