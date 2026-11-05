use crate::renderer::{pattern_spec as pattern, object_value as pattern_payload};
use crate::components::core::atoms::checkbox::CheckboxRenderer;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::{Value, ValueMap};

pub struct DataGridRenderer;

#[derive(Props, Clone, PartialEq)]
struct GridProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}


fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| format_value(value))
}

fn row_id(row: &Value, index: usize) -> String {
    row.get("id")
        .map(display)
        .unwrap_or_else(|| index.to_string())
}

fn field_value<'a>(row: &'a Value, field: &Value) -> Option<&'a Value> {
    let name = get_string(field, "name").or_else(|| get_string(field, "key"))?;
    row.get_path(&name)
}

fn field_label(field: &Value) -> String {
    get_string(field, "label")
        .or_else(|| get_string(field, "name"))
        .or_else(|| get_string(field, "key"))
        .unwrap_or_default()
}

fn text(content: String, variant: &str, color: &str, class: &str) -> Value {
    pattern(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string(variant)),
            ("color", Value::string(color)),
            ("className", Value::string(class)),
        ],
    )
}

fn empty_state(props: &Value) -> Value {
    let mut fields = vec![(
        "title",
        Value::string(get_string(props, "emptyTitle").unwrap_or_else(|| "No items".to_string())),
    )];
    if let Some(icon) = props.get("emptyIcon") {
        fields.push(("icon", icon.clone()));
    }
    if let Some(description) = props.get("emptyDescription") {
        fields.push(("description", description.clone()));
    }
    if let Some(action) = props.get("emptyAction") {
        if let Some(label) = action.get("label") {
            fields.push(("actionLabel", label.clone()));
        }
        if let Some(event) = action.get("event") {
            fields.push(("actionEvent", event.clone()));
        }
    }
    pattern("empty-state", fields)
}

fn selection_payload(ids: &[String]) -> Value {
    pattern_payload([(
        "selectedIds",
        Value::Array(
            ids.iter()
                .cloned()
                .map(Value::string)
                .collect::<Vec<_>>()
                .into(),
        ),
    )])
}


fn selected_next(current: &[String], id: &str) -> Vec<String> {
    let mut next = current.to_vec();
    if let Some(index) = next.iter().position(|selected| selected == id) {
        next.remove(index);
    } else {
        next.push(id.to_string());
    }
    next
}

fn emit_selection(sink: Option<EventSink>, event: &Option<String>, ids: &[String]) {
    if let (Some(sink), Some(event)) = (sink, event) {
        sink.call((event.clone(), selection_payload(ids)));
    }
}

fn action_button(action: &Value, row: &Value) -> Option<Value> {
    let route = crate::renderer::logged_row_route(action, row);
    let event = if action.get("navigatesTo").is_some() {
        None
    } else {
        get_string(action, "event")
    };
    if route.is_none() && event.is_none() {
        return None;
    }
    let mut fields = vec![
        ("label", Value::string(get_string_or(action, "label", ""))),
        ("actionPayload", crate::renderer::logged_row_payload(action, row)?),
        (
            "variant",
            Value::string(get_string_or(action, "variant", "ghost")),
        ),
        ("size", Value::string("sm")),
    ];
    if let Some(route) = route {
        fields.push(("href", Value::string(route)));
    }
    if let Some(event) = event {
        fields.push(("action", Value::string(event)));
    }
    if let Some(icon) = action.get("icon") {
        fields.push(("icon", icon.clone()));
    }
    Some(pattern("button", fields))
}

fn action_menu(actions: &[Value], row: &Value) -> Value {
    let items = actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let mut fields = vec![
                ("id", Value::string(index.to_string())),
                ("label", Value::string(get_string_or(action, "label", ""))),
                (
                    "variant",
                    Value::string(get_string_or(action, "variant", "default")),
                ),
            ];
            let route = crate::renderer::logged_row_route(action, row);
            if action.get("navigatesTo").is_some() && route.is_none() {
                fields.push(("disabled", Value::Bool(true)));
            } else if let Some(route) = route {
                fields.push(("href", Value::string(route)));
            } else if let Some(event) = get_string(action, "event") {
                fields.push(("event", Value::string(event)));
            }
            if let Some(icon) = action.get("icon") {
                fields.push(("icon", icon.clone()));
            }
            pattern_payload(fields)
        })
        .collect::<Vec<_>>();
    let trigger = pattern(
        "button",
        [
            ("label", Value::string("⋯")),
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("aria-label", Value::string("Actions")),
        ],
    );
    pattern(
        "menu",
        [
            ("items", Value::Array(items.into())),
            ("trigger", trigger),
            ("position", Value::string("bottom-end")),
        ],
    )
}

#[allow(non_snake_case)]
fn Grid(input: GridProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut selected = use_signal(Vec::<String>::new);
    if get_bool_or(props, "isLoading", false) {
        let skeleton = props
            .get("skeleton")
            .cloned()
            .unwrap_or_else(|| Value::string("grid"));
        let spec = pattern(
            "skeleton",
            [
                ("spec", skeleton),
                (
                    "className",
                    Value::string(get_string_or(props, "className", "")),
                ),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }
    if let Some(message) = props
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
    {
        let spec = text(message.to_string(), "body", "error", "");
        return rsx! { div { class: "text-center py-8", role: "alert", {registry.render_spec(&spec, None, &view)} } };
    }
    let rows = get_entity_rows_from(props, view.entities);
    if rows.is_empty() {
        return registry.render_spec(&empty_state(props), None, &view);
    }
    let fields = {
        let authored = get_array(props, "fields");
        if authored.is_empty() {
            let columns = get_array(props, "columns");
            if columns.is_empty() {
                get_columns(props, &rows)
            } else {
                columns
            }
        } else {
            authored
        }
    };
    let title_field = fields
        .iter()
        .find(|field| matches!(get_string(field, "variant").as_deref(), Some("h3" | "h4")));
    let actions = get_array(props, "itemActions");
    let lambda = props.get("renderItem").and_then(render_item_param_body);
    let image_field = get_string(props, "imageField");
    let item_click = get_string(props, "itemClickEvent");
    let selectable = get_bool_or(props, "selectable", false);
    let selection_event = get_string(props, "selectionEvent");
    let sink = view.sink;
    let data = &rows;
    let all_ids = data
        .iter()
        .enumerate()
        .map(|(index, row)| row_id(row, index))
        .collect::<Vec<_>>();
    let all_selected = !all_ids.is_empty() && all_ids.iter().all(|id| selected().contains(id));
    let selected_count = selected().len();
    let gap = match get_string_or(props, "gap", "md").as_str() {
        "none" => "gap-0",
        "sm" => "gap-2",
        "lg" => "gap-6",
        "xl" => "gap-8",
        _ => "gap-4",
    };
    let look = match get_string_or(props, "look", "dense").as_str() {
        "spacious" => "gap-8 [&>*]:p-card-lg",
        "striped" => "[&>*:nth-child(even)]:bg-muted/30",
        "borderless" => "[&>*]:border-0 [&>*]:shadow-none",
        "card-rows" => "[&>*]:shadow-elevation-card [&>*]:rounded-container [&>*]:border [&>*]:border-border [&>*]:p-card-md",
        _ => "gap-2 [&>*]:p-card-sm",
    };
    let cols = get_number(props, "cols")
        .filter(|cols| (1.0..=6.0).contains(cols) && cols.fract() == 0.0)
        .map(|cols| cols as usize);
    let scroll_x = get_bool_or(props, "scrollX", false);
    let as_rows = cols == Some(1) && !scroll_x && lambda.is_none();
    let cols_class = match cols {
        Some(1) => "grid-cols-1",
        Some(2) => "@[30rem]:grid-cols-2",
        Some(3) => "@[30rem]:grid-cols-2 @[42rem]:grid-cols-3",
        Some(4) => "@[30rem]:grid-cols-2 @[42rem]:grid-cols-3 @[56rem]:grid-cols-4",
        Some(5) => "@[30rem]:grid-cols-2 @[42rem]:grid-cols-3 @[64rem]:grid-cols-5",
        Some(6) => "@[30rem]:grid-cols-2 @[42rem]:grid-cols-3 @[72rem]:grid-cols-6",
        _ => "",
    };
    let min_card_width = get_number_or(props, "minCardWidth", 280.0).max(0.0);
    let style = if scroll_x {
        format!("grid-auto-flow:column;grid-auto-columns:minmax(min({min_card_width}px,85%),1fr)")
    } else if cols.is_none() {
        format!("grid-template-columns:repeat(auto-fit,minmax(min({min_card_width}px,100%),1fr))")
    } else {
        String::new()
    };
    let layout = if as_rows {
        "flex flex-col divide-y divide-border"
    } else {
        "grid"
    };
    let scroll_class = if scroll_x {
        "grid-flow-col overflow-x-auto snap-x snap-mandatory [&>*]:snap-start pb-2"
    } else {
        ""
    };
    let grid_class = merge_class_name(
        &format!("{layout} {gap} {look} {cols_class} {scroll_class}"),
        props,
    );
    let item_enter = get_string(props, "itemEnter")
        .filter(|value| value != "none")
        .map(|value| format!("almadar-enter-{value}"))
        .unwrap_or_default();
    let infinite_scroll = get_bool_or(props, "infiniteScroll", false);
    let sentinel = get_string(props, "loadMoreEvent")
        .filter(|_| infinite_scroll)
        .map(|event| {
            pattern(
                "infinite-scroll-sentinel",
                [
                    ("loadMoreEvent", Value::string(event)),
                    ("hasMore", Value::Bool(get_bool_or(props, "hasMore", true))),
                    ("isLoading", Value::Bool(false)),
                ],
            )
        });
    let toolbar_checkbox = pattern(
        "checkbox",
        [
            ("checked", Value::Bool(all_selected)),
            ("aria-label", Value::string("Select all")),
        ],
    );
    let toolbar_selection_event = selection_event.clone();
    let toolbar_change: Callback<bool> = Callback::new(move |_| {
        let next = if all_selected {
            Vec::new()
        } else {
            all_ids.clone()
        };
        selected.set(next.clone());
        emit_selection(sink, &toolbar_selection_event, &next);
    });
    rsx! {
        div { class: "@container w-full min-w-0 flex flex-col gap-2", ..shared_root_attrs(props),
            if selectable && selected_count > 0 {
                div { class: "flex items-center gap-2 px-2 py-2 bg-muted rounded-container",
                    {CheckboxRenderer.render_with_change(&toolbar_checkbox, &view, Some(toolbar_change))}
                    span { class: "text-xs font-semibold", "{selected_count} selected" }
                }
            }
            div { class: "{grid_class}", style: "{style}", "data-grid-layout": if as_rows { "rows" } else { "cards" },
                for (index, row) in data.iter().enumerate() {
                    {
                        let id = row_id(row, index);
                        let is_selected = selected().contains(&id);
                        let row_for_click = row.clone();
                        let click_event = item_click.clone();
                        let custom = lambda.as_ref().map(|(param, body)| substitute_row_bindings(body, param, row));
                        let title = title_field.and_then(|field| field_value(row, field)).map(display);
                        let title_variant = if title_field.and_then(|field| get_string(field, "variant")).as_deref() == Some("h3") && !as_rows { "h3" } else { "h4" };
                        let title_spec = title.as_ref().map(|title| text(title.clone(), title_variant, "primary", "font-semibold min-w-0 line-clamp-2 break-words"));
                        let image_value = image_field.as_deref().and_then(|field| row.get_path(field));
                        let image_url = image_value.and_then(|value| value.as_str().or_else(|| value.get("url").and_then(Value::as_str)));
                        let image_spec = image_url.map(|url| pattern("image", [("src", Value::string(url)), ("alt", Value::string(title.clone().unwrap_or_default())), ("rounded", Value::string("none")), ("className", Value::string("w-full h-full object-cover"))]));
                        let placeholder = pattern("icon", [("name", Value::string("image")), ("size", Value::string(if as_rows { "sm" } else { "lg" }))]);
                        let row_actions = actions.iter().filter(|action| row_action_is_shown(action, row, None)).cloned().collect::<Vec<_>>();
                        let inline_limit = get_number_or(props, "maxInlineActions", 1.0) > 0.0;
                        let inline_index = if inline_limit { row_actions.iter().position(|action| get_string(action, "variant").as_deref() == Some("primary") && action_button(action, row).is_some()) } else { None };
                        let inline_action = inline_index.and_then(|index| action_button(&row_actions[index], row));
                        let menu_actions = row_actions.iter().enumerate().filter(|(index, _)| Some(*index) != inline_index).map(|(_, action)| action.clone()).collect::<Vec<_>>();
                        let menu = if menu_actions.is_empty() { None } else { Some(action_menu(&menu_actions, row)) };
                        let menu_view = sink.map(|sink| {
                            let row_for_menu = row.clone();
                            let actions_for_menu = menu_actions.clone();
                            let relay: EventSink = Callback::new(move |(event, payload): (String, Value)| {
                                let Some(index) = payload.get("itemId").and_then(Value::as_str).and_then(|id| id.parse::<usize>().ok()) else { return; };
                                let Some(action) = actions_for_menu.get(index) else { return; };
                                if let Some(payload) = crate::renderer::logged_row_payload(action, &row_for_menu) { sink.call((event, payload)); }
                            });
                            view.for_source(view.source_trait.clone(), relay)
                        });
                        let row_selected = selected;
                        let selection_sink = sink;
                        let selection_name = selection_event.clone();
                        let selection_id = id.clone();
                        let checkbox = pattern("checkbox", [("checked", Value::Bool(is_selected)), ("aria-label", Value::string(format!("Select {}", title.clone().unwrap_or_else(|| "item".to_string()))))]);
                        let change: Callback<bool> = Callback::new(move |_| {
                            let next = selected_next(&row_selected.peek(), &selection_id);
                            let mut selected = row_selected;
                            selected.set(next.clone());
                            emit_selection(selection_sink, &selection_name, &next);
                        });
                        let row_class = if as_rows { "flex flex-row items-center gap-3 px-card-md transition-colors duration-fast hover:bg-muted/50" } else { "rounded-container border border-border hover:shadow-elevation-dialog hover:border-primary transition-all flex flex-col" };
                        let selected_class = if is_selected { if as_rows { "bg-primary/5" } else { "ring-2 ring-primary border-primary" } } else { "" };
                        let clickable = if click_event.is_some() { "cursor-pointer" } else { "" };
                        rsx! { div { key: "{id}", class: "relative {row_class} {selected_class} {clickable} {item_enter}", "data-entity-row": "", "data-entity-id": "{id}", aria_current: if is_selected { Some("true") } else { None },
                            onclick: move |_| {
                                if let (Some(sink), Some(event)) = (sink, &click_event) {
                                    let empty = Value::Object(ValueMap::new().into());
                                    if let Some(payload) = crate::renderer::logged_row_payload(&empty, &row_for_click) { sink.call((event.clone(), payload)); }
                                }
                            },
                            if let Some(custom) = custom.as_ref() {
                                {registry.render_spec(custom, None, &view)}
                            } else {
                                if image_field.is_some() {
                                    div { class: if as_rows { "w-12 h-12 flex-shrink-0 overflow-hidden rounded-container" } else { "w-full aspect-video overflow-hidden rounded-t-container" },
                                        if let Some(image) = image_spec { {registry.render_spec(&image, None, &view)} }
                                        else { div { class: "flex items-center justify-center w-full h-full bg-muted text-muted-foreground", "data-media-placeholder": "", {registry.render_spec(&placeholder, None, &view)} } }
                                    }
                                }
                                div { class: if as_rows { "flex-1 min-w-0 py-card-sm" } else { "p-4" },
                                    div { class: "flex items-start justify-between gap-2",
                                        if selectable {
                                            div { onclick: move |event| event.stop_propagation(),
                                                {CheckboxRenderer.render_with_change(&checkbox, &view, Some(change))}
                                            }
                                        }
                                        div { class: "flex-1 min-w-0 flex flex-col gap-1",
                                            if let Some(title) = title_spec { {registry.render_spec(&title, None, &view)} }
                                            for field in fields.iter().filter(|field| get_string(field, "variant").as_deref() == Some("badge")) {
                                                {render_badge(field, row, registry, &view)}
                                            }
                                        }
                                    }
                                }
                                {render_body(&fields, title_field, row, registry, &view, as_rows)}
                            }
                            if inline_action.is_some() || menu.is_some() {
                                div { class: "flex flex-wrap gap-1 px-4 pb-3", onclick: move |event| event.stop_propagation(),
                                    if let Some(action) = inline_action { {registry.render_spec(&action, None, &view)} }
                                    if let Some(menu) = menu { {registry.render_spec(&menu, None, menu_view.as_ref().unwrap_or(&view))} }
                                }
                            }
                        } }
                    }
                }
            }
            if let Some(sentinel) = sentinel { {registry.render_spec(&sentinel, None, &view)} }
        }
    }
}

fn render_badge(
    field: &Value,
    row: &Value,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let Some(value) = field_value(row, field) else {
        return rsx! {};
    };
    let value = display(value);
    let label = field
        .get("labels")
        .and_then(|labels| labels.get(&value))
        .and_then(Value::as_str)
        .unwrap_or(&value);
    let variant = field
        .get("colorMap")
        .and_then(|colors| colors.get(&value))
        .and_then(Value::as_str)
        .unwrap_or("default");
    let variant = if variant == "destructive" {
        "danger"
    } else {
        variant
    };
    let spec = pattern(
        "badge",
        [
            ("label", Value::string(label)),
            ("variant", Value::string(variant)),
        ],
    );
    registry.render_spec(&spec, None, view)
}

fn render_body(
    fields: &[Value],
    title_field: Option<&Value>,
    row: &Value,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    as_rows: bool,
) -> Element {
    let content = fields
        .iter()
        .filter(|field| {
            title_field != Some(*field) && get_string(field, "variant").as_deref() != Some("badge")
        })
        .filter_map(|field| {
            let value = field_value(row, field)?;
            if value == &Value::Null || value.as_str() == Some("") {
                return None;
            }
            let variant = get_string_or(field, "variant", "body");
            let label = field_label(field);
            if variant == "progress" {
                let number = value.as_number()?;
                let spec = pattern(
                    "progress-bar",
                    [
                        ("value", Value::Number(number)),
                        ("max", Value::Number(100.0)),
                        ("label", Value::string(label)),
                    ],
                );
                return Some(registry.render_spec(&spec, None, view));
            }
            if variant == "caption" && get_string(field, "format").as_deref() != Some("boolean") {
                let spec = text(display(value), "small", "secondary", "line-clamp-2");
                return Some(registry.render_spec(&spec, None, view));
            }
            let text_value = if get_string(field, "format").as_deref() == Some("boolean") {
                if value.as_bool() == Some(true) {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            } else {
                display(value)
            };
            let label_spec = text(format!("{label}:"), "caption", "secondary", "");
            let value_spec = text(text_value, "small", "secondary", "");
            Some(rsx! { div { class: "flex items-center gap-2",
                {registry.render_spec(&label_spec, None, view)}
                {registry.render_spec(&value_spec, None, view)}
            } })
        })
        .collect::<Vec<_>>();
    if content.is_empty() {
        return rsx! {};
    }
    let class = if as_rows {
        "basis-full min-w-0"
    } else {
        "px-4 pt-2 pb-4 flex-1"
    };
    rsx! { div { class: "{class} flex flex-col gap-1", for item in content { {item} } } }
}

impl DioxusPatternRenderer for DataGridRenderer {
    fn pattern_name(&self) -> &str {
        "data-grid"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Grid { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
