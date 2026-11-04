use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct DataListRenderer;

fn pattern(name: &str, fields: impl IntoIterator<Item = (String, Value)>) -> Value {
    let mut props = orbital_core::ValueMap::new();
    props.insert("type".into(), Value::string(name));
    for (key, value) in fields {
        props.insert(key.into(), value);
    }
    Value::Object(props.into())
}

fn empty_pattern(props: &Value) -> Value {
    let mut fields = vec![(
        "title".to_string(),
        Value::string(
            get_string(props, "emptyTitle")
                .or_else(|| get_string(props, "emptyMessage"))
                .unwrap_or_else(|| "No items".to_string()),
        ),
    )];
    if let Some(icon) = props.get("emptyIcon") {
        fields.push(("icon".to_string(), icon.clone()));
    }
    if let Some(description) = props.get("emptyDescription") {
        fields.push(("description".to_string(), description.clone()));
    }
    if let Some(action) = props.get("emptyAction") {
        if let Some(label) = action.get("label") {
            fields.push(("actionLabel".to_string(), label.clone()));
        }
        if let Some(event) = action.get("event") {
            fields.push(("actionEvent".to_string(), event.clone()));
        }
    }
    pattern("empty-state", fields)
}

fn action_pattern(action: &Value, row: &Value) -> Option<Value> {
    let route = row_action_route(action, row, None).ok().flatten();
    let event = if action.get("navigatesTo").is_some() {
        None
    } else {
        get_string(action, "event")
    };
    if route.is_none() && event.is_none() {
        return None;
    }
    let mut fields = vec![
        (
            "label".to_string(),
            Value::string(get_string_or(action, "label", "")),
        ),
        ("size".to_string(), Value::string("sm")),
        (
            "variant".to_string(),
            Value::string(get_string_or(action, "variant", "ghost")),
        ),
        (
            "actionPayload".to_string(),
            row_action_payload(action, row, None),
        ),
    ];
    if let Some(route) = route {
        fields.push(("href".to_string(), Value::string(route)));
    }
    if let Some(event) = event {
        fields.push(("action".to_string(), Value::string(event)));
    }
    if let Some(icon) = action.get("icon") {
        fields.push(("icon".to_string(), icon.clone()));
    }
    Some(pattern("button", fields))
}

impl DioxusPatternRenderer for DataListRenderer {
    fn pattern_name(&self) -> &str {
        "data-list"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        if get_bool_or(props, "isLoading", false) {
            let spec = props
                .get("skeleton")
                .cloned()
                .unwrap_or_else(|| Value::string("list"));
            return registry.render_spec(
                &pattern("skeleton", [("spec".to_string(), spec)]),
                None,
                view,
            );
        }
        if let Some(message) = props
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
        {
            let spec = pattern(
                "typography",
                [
                    ("text".to_string(), Value::string(message)),
                    ("color".to_string(), Value::string("error")),
                ],
            );
            return rsx! { div { class: "text-center py-8", role: "alert", {registry.render_spec(&spec, None, view)} } };
        }
        let rows = get_entity_rows_from(props, view.entities);
        if rows.is_empty() {
            return registry.render_spec(&empty_pattern(props), None, view);
        }
        let lambda = props.get("renderItem").and_then(render_item_param_body);
        let columns = get_columns(props, &rows);
        let actions = get_array(props, "itemActions");
        let item_click = get_string(props, "itemClickEvent");
        let sink = view.sink;
        let variant = get_string_or(props, "variant", "default");
        let row_class = match variant.as_str() {
            "card" => "rounded-interactive border border-border bg-card p-4",
            "compact" => "border-b border-border px-3 py-1",
            _ => "border-b border-border px-4 py-3",
        };
        let gap = match get_string_or(props, "gap", "none").as_str() {
            "sm" => "gap-1",
            "md" => "gap-2",
            "lg" => "gap-4",
            _ => "",
        };
        let class = merge_class_name(&format!("almadar-data-list flex flex-col {gap}"), props);
        rsx! { div { class: "{class}", ..shared_root_attrs(props),
            for (index, row) in rows.iter().enumerate() {
                {
                    let row = row.clone();
                    let row_for_click = row.clone();
                    let row_id = row.get("id").map(format_value).unwrap_or_else(|| index.to_string());
                    let row_actions = actions.iter().filter(|action| row_action_is_shown(action, &row, None)).filter_map(|action| action_pattern(action, &row)).collect::<Vec<_>>();
                    let custom = lambda.as_ref().map(|(param, body)| substitute_row_bindings(body, param, &row));
                    let click = item_click.clone();
                    rsx! { div { key: "{row_id}", class: "{row_class} relative", "data-entity-row": "", "data-entity-id": "{row_id}",
                        onclick: move |_| {
                            if let (Some(sink), Some(event)) = (sink, &click) {
                                let empty = Value::Object(orbital_core::ValueMap::new().into());
                                sink.call((event.clone(), row_action_payload(&empty, &row_for_click, None)));
                            }
                        },
                        if let Some(custom) = custom.as_ref() { {registry.render_spec(custom, None, view)} }
                        else { div { class: "flex flex-col gap-1",
                            for column in columns.iter() {
                                {
                                    let key = get_string(column, "key").or_else(|| get_string(column, "name")).unwrap_or_default();
                                    let value = row.get(&key).map(format_value).unwrap_or_default();
                                    rsx! { span { key: "{key}", class: "text-sm text-foreground", "{value}" } }
                                }
                            }
                        } }
                        if !row_actions.is_empty() { div { class: "flex items-center gap-1 mt-2", onclick: move |event| event.stop_propagation(),
                            for (action_index, action) in row_actions.iter().enumerate() {
                                div { key: "{action_index}", {registry.render_spec(action, None, view)} }
                            }
                        } }
                    } }
                }
            }
        } }
    }
}
