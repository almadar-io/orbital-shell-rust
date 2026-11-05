use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    array_value, cn, format_cell_text, format_display_value, logged_row_payload, logged_row_route,
    nested_value, object_value, pattern_spec as pattern, plain_text, typography_spec,
    DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct DataListRenderer;

const STRETCHED_CONTROL: &str = "after:absolute after:inset-0 after:content-[''] after:rounded-[inherit] focus-visible:outline-none focus-visible:after:ring-2 focus-visible:after:ring-primary focus-visible:after:ring-inset";
const HSTACK_XS: &str = "flex flex-row gap-1 items-center justify-start";

struct Field {
    name: String,
    label: Option<String>,
    variant: Option<String>,
    format: Option<String>,
    icon: Option<String>,
    color_map: Option<Value>,
    labels: Option<Value>,
}

fn fields_of(props: &Value) -> Vec<Field> {
    let defs = ["fields", "columns"]
        .iter()
        .find_map(|name| {
            props
                .get(name)
                .and_then(Value::as_array)
                .map(|items| items.to_vec())
        })
        .unwrap_or_default();
    defs.iter()
        .filter_map(|def| {
            def.as_object()?;
            Some(Field {
                name: get_string(def, "name").or_else(|| get_string(def, "key"))?,
                label: get_string(def, "label").or_else(|| get_string(def, "header")),
                variant: get_string(def, "variant"),
                format: get_string(def, "format"),
                icon: get_string(def, "icon"),
                color_map: def.get("colorMap").cloned(),
                labels: def.get("labels").cloned(),
            })
        })
        .collect()
}

fn relation_options(props: &Value, field: &str) -> Option<Vec<Value>> {
    props
        .get("relationsData")?
        .get(field)?
        .as_array()
        .map(|items| items.to_vec())
}

fn badge_variant(raw: &str, color_map: Option<&Value>) -> String {
    match color_map.and_then(|map| get_string(map, raw)) {
        Some(color) if color == "destructive" => "danger".to_string(),
        Some(color) => color,
        None => "default".to_string(),
    }
}

fn label_for(text: String, labels: Option<&Value>) -> String {
    labels
        .and_then(|labels| get_string(labels, &text))
        .unwrap_or(text)
}

fn empty_pattern(props: &Value) -> Value {
    let mut fields = vec![(
        "title".to_string(),
        Value::string(
            get_string(props, "emptyTitle")
                .filter(|text| !text.is_empty())
                .or_else(|| get_string(props, "emptyMessage").filter(|text| !text.is_empty()))
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

fn action_pattern(action: &Value, row: &Value, on_primary: bool) -> Option<Value> {
    let route = logged_row_route(action, row);
    let event = if action.get("navigatesTo").is_some() {
        None
    } else {
        get_string(action, "event")
    };
    if route.is_none() && event.is_none() {
        return None;
    }
    let variant = get_string_or(action, "variant", "ghost");
    let class = cn(&[
        if variant == "danger" {
            "text-foreground hover:bg-error/10"
        } else {
            ""
        },
        if on_primary {
            "!text-primary-foreground hover:bg-primary-foreground/15"
        } else {
            ""
        },
    ]);
    let mut fields = vec![
        (
            "label".to_string(),
            Value::string(get_string_or(action, "label", "")),
        ),
        ("size".to_string(), Value::string("sm")),
        ("variant".to_string(), Value::string(variant)),
        (
            "actionPayload".to_string(),
            logged_row_payload(action, row)?,
        ),
    ];
    if !class.is_empty() {
        fields.push(("className".to_string(), Value::string(class)));
    }
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

#[derive(Props, Clone, PartialEq)]
struct DataListBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn DataListBody(p: DataListBodyProps) -> Element {
    let props = &p.props;
    let registry = &p.registry;
    let page_size = match props.get("pageSize") {
        Some(_) => get_number_or(props, "pageSize", 5.0).max(0.0) as usize,
        None => 5,
    };
    let mut visible = use_signal(|| if page_size > 0 { page_size } else { usize::MAX });
    let base = p.view.as_view();
    let sink = base.sink;

    let all_rows = get_entity_rows_from(props, base.entities);
    let total = all_rows.len();
    let shown = if page_size > 0 {
        visible().min(total)
    } else {
        total
    };
    let rows: Vec<Value> = all_rows.into_iter().take(shown).collect();
    let has_more_local = page_size > 0 && shown < total;
    let actions = get_array(props, "itemActions");
    let max_inline = props
        .get("maxInlineActions")
        .and_then(|_| get_number(props, "maxInlineActions"))
        .map(|count| count.max(0.0) as usize);

    let relay: EventSink = {
        let rows = rows.clone();
        let actions = actions.clone();
        Callback::new(move |(event, payload): (String, Value)| {
            if event == "__dl_more" {
                let step = if page_size > 0 { page_size } else { 5 };
                visible.set(visible().saturating_add(step));
            } else if let Some(target) = event.strip_prefix("__dl_act:") {
                let mut parts = target.split(':');
                let row_index = parts.next().and_then(|part| part.parse::<usize>().ok());
                let action_index = parts.next().and_then(|part| part.parse::<usize>().ok());
                if let (Some(sink), Some(row), Some(action)) = (
                    sink,
                    row_index.and_then(|index| rows.get(index)),
                    action_index.and_then(|index| actions.get(index)),
                ) {
                    if let (Some(name), Some(payload)) =
                        (get_string(action, "event"), logged_row_payload(action, row))
                    {
                        sink.call((name, payload));
                    }
                }
            } else if let Some(sink) = sink {
                sink.call((event, payload));
            }
        })
    };
    let view = base.for_source(base.source_trait.clone(), relay);
    let render = |spec: Value| registry.render_spec(&spec, None, &view);
    let class_name = get_string_or(props, "className", "");

    if get_bool_or(props, "isLoading", false) {
        let spec = props
            .get("skeleton")
            .cloned()
            .unwrap_or_else(|| Value::string("list"));
        let mut fields = vec![("spec", spec)];
        if !class_name.is_empty() {
            fields.push(("className", Value::string(class_name)));
        }
        return render(pattern("skeleton", fields));
    }
    if let Some(message) = props
        .get("error")
        .and_then(|error| get_string(error, "message"))
    {
        let text = render(typography_spec("body", Some("error"), "", &message));
        return rsx! { div { class: "text-center py-8", {text} } };
    }
    if rows.is_empty() {
        return render(empty_pattern(props));
    }

    let fields = fields_of(props);
    let title_field = fields
        .iter()
        .find(|field| matches!(field.variant.as_deref(), Some("h3" | "h4")));
    let is_role = |field: &Field, role: &str| field.variant.as_deref() == Some(role);
    let is_title = |field: &Field| title_field.is_some_and(|title| std::ptr::eq(title, field));
    let avatar_field = fields.iter().find(|field| is_role(field, "avatar"));
    let overline_fields: Vec<&Field> = fields
        .iter()
        .filter(|field| is_role(field, "overline"))
        .collect();
    let badge_fields: Vec<&Field> = fields
        .iter()
        .filter(|field| is_role(field, "badge") && !is_title(field))
        .collect();
    let progress_fields: Vec<&Field> = fields
        .iter()
        .filter(|field| is_role(field, "progress"))
        .collect();
    let prose_fields: Vec<&Field> = fields
        .iter()
        .filter(|field| is_role(field, "caption") && field.format.as_deref() != Some("boolean"))
        .collect();
    let body_fields: Vec<&Field> = fields
        .iter()
        .filter(|field| {
            !is_title(field)
                && !is_role(field, "avatar")
                && !is_role(field, "overline")
                && !(is_role(field, "badge"))
                && !is_role(field, "progress")
                && !(is_role(field, "caption") && field.format.as_deref() != Some("boolean"))
        })
        .collect();

    let variant = get_string_or(props, "variant", "default");
    let is_card = variant == "card";
    let is_compact = variant == "compact";
    let is_message = variant == "message";
    let look = get_string_or(props, "look", "dense");
    let item_click = get_string(props, "itemClickEvent");
    let enter = match get_string(props, "itemEnter").as_deref() {
        Some(kind @ ("fade" | "rise" | "scale" | "slide")) => format!("almadar-enter-{kind}"),
        _ => String::new(),
    };
    let render_item = props.get("renderItem").and_then(render_item_lambda);

    let row_actions: Vec<Vec<usize>> = rows
        .iter()
        .map(|row| {
            actions
                .iter()
                .enumerate()
                .filter(|(_, action)| row_action_is_shown(action, row, None))
                .map(|(index, _)| index)
                .collect()
        })
        .collect();

    let actions_el = |ri: usize, only: &[usize], on_primary: bool| -> Option<Element> {
        if only.is_empty() {
            return None;
        }
        let row = &rows[ri];
        let inline_count = max_inline.map_or(only.len(), |max| max.min(only.len()));
        let inline = &only[..inline_count];
        let overflow = &only[inline_count..];
        let buttons: Vec<Element> = inline
            .iter()
            .filter_map(|&ai| action_pattern(&actions[ai], row, on_primary))
            .map(|spec| render(spec))
            .collect();
        let menu = (!overflow.is_empty()).then(|| {
            let items: Vec<Value> = overflow
                .iter()
                .map(|&ai| {
                    let action = &actions[ai];
                    let mut item = vec![
                        ("id", Value::string(ai.to_string())),
                        ("label", Value::string(get_string_or(action, "label", ""))),
                        (
                            "variant",
                            Value::string(
                                if get_string(action, "variant").as_deref() == Some("danger") {
                                    "danger"
                                } else {
                                    "default"
                                },
                            ),
                        ),
                    ];
                    if let Some(icon) = action.get("icon") {
                        item.push(("icon", icon.clone()));
                    }
                    match logged_row_route(action, row) {
                        Some(route) => item.push(("href", Value::string(route))),
                        None => item.push(("event", Value::string(format!("__dl_act:{ri}:{ai}")))),
                    }
                    object_value(item)
                })
                .collect();
            render(pattern(
                "menu",
                [
                    ("position", Value::string("bottom-end")),
                    (
                        "trigger",
                        pattern(
                            "button",
                            [
                                ("variant", Value::string("ghost")),
                                ("size", Value::string("sm")),
                                ("icon", Value::string("more-horizontal")),
                                ("label", Value::string("")),
                                ("aria-label", Value::string("Actions")),
                            ],
                        ),
                    ),
                    ("items", array_value(items)),
                ],
            ))
        });
        let stop = item_click.is_some();
        Some(rsx! {
            div { class: "{HSTACK_XS} flex-shrink-0 relative z-[1]",
                onclick: move |event| { if stop { event.stop_propagation(); } },
                for button in buttons.iter() { {button.clone()} }
                if let Some(menu) = menu { {menu} }
            }
        })
    };

    let row_open = |row: &Value| -> Option<(String, Value)> {
        let event = item_click.clone()?;
        Some((event, logged_row_payload(&object_value::<&str>([]), row)?))
    };
    let row_id = |ri: usize| -> String {
        rows[ri]
            .get("id")
            .filter(|id| !id.is_null())
            .map(plain_text)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| ri.to_string())
    };

    let surface_class = if look == "card-rows" || get_string_or(props, "surface", "auto") != "auto"
    {
        ""
    } else {
        "surface-content overflow-hidden"
    };
    let gap_class = match get_string_or(props, "gap", "none").as_str() {
        "sm" => "gap-1",
        "md" => "gap-2",
        "lg" => "gap-4",
        _ => "",
    };
    let look_class = match look.as_str() {
        "spacious" => "[&_[data-entity-row]>div]:!py-5 [&_[data-entity-row]>div]:!px-8",
        "striped" => "[&_[data-entity-row]:nth-child(even)>div]:bg-muted/30",
        "borderless" => "[&_[data-entity-row]>div]:!border-0 [&_[data-entity-row]>div]:!hover:border-transparent",
        "card-rows" => "[&_[data-entity-row]>div]:surface-content [&_[data-entity-row]]:mb-2",
        _ => "[&_[data-entity-row]>div]:!py-1 [&_[data-entity-row]>div]:!px-3",
    };

    let more = has_more_local.then(|| {
        render(pattern(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("chevron-down")),
                (
                    "label",
                    Value::string(format!("Show More ({} remaining)", total - shown)),
                ),
                ("action", Value::string("__dl_more")),
            ],
        ))
    });

    if is_message {
        let sender_field = get_string(props, "senderField");
        let sender_label_field = get_string(props, "senderLabelField");
        let current_user = get_string(props, "currentUser");
        let content_field = fields
            .iter()
            .find(|field| is_role(field, "body"))
            .or(title_field)
            .map(|field| field.name.clone())
            .unwrap_or_default();
        let timestamp_field = fields
            .iter()
            .find(|field| field.format.as_deref() == Some("date"));
        let elements: Vec<Element> = rows
            .iter()
            .enumerate()
            .map(|(ri, row)| {
                let id = row_id(ri);
                let sender = sender_field
                    .as_ref()
                    .and_then(|name| nested_value(row, name))
                    .map(|value| plain_text(&value))
                    .unwrap_or_default();
                let is_sent = current_user.as_ref().is_some_and(|user| !user.is_empty() && *user == sender);
                let content = nested_value(row, &content_field)
                    .filter(|value| !value.is_null())
                    .map(|value| plain_text(&value))
                    .unwrap_or_default();
                let sender_text = sender_label_field
                    .as_ref()
                    .and_then(|name| nested_value(row, name))
                    .filter(|value| !value.is_null() && !matches!(value, Value::String(text) if text.is_empty()))
                    .map(|value| plain_text(&value))
                    .unwrap_or_else(|| sender.clone());
                let timestamp = timestamp_field
                    .and_then(|field| nested_value(row, &field.name))
                    .filter(|value| !value.is_null())
                    .map(|value| format_display_value(&value, Some("date")));
                let meta: Vec<Element> = fields
                    .iter()
                    .filter(|field| {
                        field.name != content_field
                            && Some(field.name.as_str()) != timestamp_field.map(|f| f.name.as_str())
                    })
                    .filter_map(|field| {
                        let value = nested_value(row, &field.name)?;
                        if value.is_null() || matches!(&value, Value::String(text) if text.is_empty()) {
                            return None;
                        }
                        let options = relation_options(props, &field.name);
                        let text = format_cell_text(&value, field.format.as_deref(), options.as_deref());
                        if is_role(field, "badge") {
                            Some(render(pattern(
                                "badge",
                                [
                                    ("variant", Value::string(badge_variant(&plain_text(&value), field.color_map.as_ref()))),
                                    ("label", Value::string(label_for(text, field.labels.as_ref()))),
                                ],
                            )))
                        } else {
                            Some(render(typography_spec(
                                "caption",
                                None,
                                &cn(&["text-xs", if is_sent { "opacity-70" } else { "text-muted-foreground" }]),
                                &text,
                            )))
                        }
                    })
                    .collect();
                let timestamp_el = timestamp.map(|text| {
                    render(typography_spec(
                        "caption",
                        None,
                        &cn(&["text-xs whitespace-nowrap", if is_sent { "opacity-70" } else { "text-muted-foreground" }]),
                        &text,
                    ))
                });
                let sender_el = (!is_sent && sender_field.is_some())
                    .then(|| render(typography_spec("caption", None, "font-semibold mb-0.5", &sender_text)));
                let body_el = render(typography_spec(
                    "body",
                    None,
                    if is_sent { "text-primary-foreground" } else { "" },
                    &content,
                ));
                let actions = actions_el(ri, &row_actions[ri], is_sent);
                let open = row_open(row);
                let has_open = open.is_some();
                let row_class = cn(&[
                    &enter,
                    "relative flex px-4 group/rowactions",
                    if has_open { "cursor-pointer" } else { "" },
                    if is_sent { "justify-end" } else { "justify-start" },
                ]);
                let bubble_class = cn(&[
                    "max-w-[75%] px-4 py-2",
                    if is_sent {
                        "bg-primary text-primary-foreground rounded-container rounded-ee-sm"
                    } else {
                        "bg-muted text-foreground rounded-container rounded-es-sm"
                    },
                ]);
                rsx! {
                    div { key: "{id}", "data-entity-row": "", "data-entity-id": "{id}", "data-item-key": "{id}", class: "{row_class}",
                        onclick: move |_| {
                            if let (Some(sink), Some((name, payload))) = (sink, &open) {
                                sink.call((name.clone(), payload.clone()));
                            }
                        },
                        div { class: "{bubble_class}",
                            if let Some(sender_el) = sender_el { {sender_el} }
                            div { role: has_open.then_some("button"), tabindex: has_open.then_some("0"),
                                class: if has_open { STRETCHED_CONTROL } else { "" },
                                {body_el}
                            }
                            if !meta.is_empty() {
                                div { class: "flex flex-row gap-1 items-stretch justify-start mt-1 flex-wrap",
                                    for item in meta.iter() { {item.clone()} }
                                }
                            }
                            div { class: "flex flex-row gap-1 items-stretch justify-start mt-1 items-center justify-between flex-wrap",
                                if let Some(timestamp_el) = timestamp_el { {timestamp_el} } else { span {} }
                                if let Some(actions) = actions { {actions} }
                            }
                        }
                    }
                }
            })
            .collect();
        return rsx! {
            div { class: "flex flex-col gap-2 items-stretch justify-start py-2 {class_name}",
                for item in elements.iter() { {item.clone()} }
                if let Some(more) = more { div { class: "flex justify-center py-3", {more} } }
            }
        };
    }

    let last_index = rows.len() - 1;
    let elements: Vec<Element> = rows
        .iter()
        .enumerate()
        .map(|(ri, row)| {
            let id = row_id(ri);
            let open = row_open(row);
            let has_open = open.is_some();
            let divider = (is_card || is_compact) && ri != last_index;
            let divider_class = cn(&["border-b border-border/40", if is_compact { "mx-4" } else { "mx-6" }]);
            let outer_click = open.clone();

            if let Some(lambda) = &render_item {
                let custom = render(lambda.apply(row, ri));
                let primary: Vec<usize> = row_actions[ri]
                    .iter()
                    .copied()
                    .filter(|&ai| get_string(&actions[ai], "variant").as_deref() == Some("primary"))
                    .collect();
                let others: Vec<usize> = row_actions[ri]
                    .iter()
                    .copied()
                    .filter(|&ai| get_string(&actions[ai], "variant").as_deref() != Some("primary"))
                    .collect();
                let primary_el = actions_el(ri, &primary, false);
                let others_el = actions_el(ri, &others, false);
                let coarse_menu = (!others.is_empty()).then(|| {
                    let items: Vec<Value> = others
                        .iter()
                        .map(|&ai| {
                            let action = &actions[ai];
                            let mut item = vec![
                                ("id", Value::string(ai.to_string())),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                            ];
                            if let Some(icon) = action.get("icon") {
                                item.push(("icon", icon.clone()));
                            }
                            match logged_row_route(action, row) {
                                Some(route) => item.push(("href", Value::string(route))),
                                None => item.push(("event", Value::string(format!("__dl_act:{ri}:{ai}")))),
                            }
                            object_value(item)
                        })
                        .collect();
                    render(pattern(
                        "menu",
                        [
                            ("position", Value::string("bottom-end")),
                            (
                                "trigger",
                                pattern(
                                    "button",
                                    [
                                        ("variant", Value::string("ghost")),
                                        ("size", Value::string("sm")),
                                        ("icon", Value::string("more-horizontal")),
                                        ("label", Value::string("")),
                                    ],
                                ),
                            ),
                            ("items", array_value(items)),
                        ],
                    ))
                });
                let overlay_click = open.clone();
                let row_class = cn(&[
                    "relative group/rowactions",
                    if has_open { "cursor-pointer" } else { "" },
                    &enter,
                ]);
                return rsx! {
                    div { key: "{id}", "data-entity-row": "", "data-entity-id": "{id}", "data-item-key": "{id}", class: "{row_class}",
                        onclick: move |_| {
                            if let (Some(sink), Some((name, payload))) = (sink, &outer_click) {
                                sink.call((name.clone(), payload.clone()));
                            }
                        },
                        if has_open {
                            div { role: "button", tabindex: "0", "aria-label": format!("Open item {}", ri + 1),
                                class: "absolute inset-0 rounded-[inherit] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    if let (Some(sink), Some((name, payload))) = (sink, &overlay_click) {
                                        sink.call((name.clone(), payload.clone()));
                                    }
                                },
                            }
                        }
                        div { class: if has_open { "relative" } else { "" }, {custom} }
                        if let Some(primary_el) = primary_el {
                            div { class: "flex flex-row gap-2 items-stretch justify-end relative px-4 pb-3 relative z-[1]", {primary_el} }
                        }
                        if let Some(others_el) = others_el {
                            div { onclick: move |event| event.stop_propagation(),
                                class: "absolute top-2 right-2 z-10 opacity-0 group-hover/rowactions:opacity-100 focus-within:opacity-100 [@media(pointer:coarse)]:opacity-100 transition-opacity duration-fast",
                                div { class: "rounded-container border border-border bg-card/95 backdrop-blur-sm shadow-elevation-popover surface-material p-0.5 [@media(pointer:coarse)]:hidden", {others_el} }
                                div { class: "hidden [@media(pointer:coarse)]:block rounded-container border border-border bg-card/95 backdrop-blur-sm shadow-elevation-popover surface-material p-0.5",
                                    if let Some(coarse_menu) = coarse_menu { {coarse_menu} }
                                }
                            }
                        }
                        if divider && is_card { div { class: "{divider_class}" } }
                    }
                };
            }

            let options_for = |field: &Field| relation_options(props, &field.name);
            let title_value = title_field.and_then(|field| nested_value(row, &field.name));
            let title_text = title_value.as_ref().and_then(|value| {
                title_field
                    .and_then(|field| relation_cell_text(value, options_for(field).as_deref()))
                    .or_else(|| (!value.is_null()).then(|| plain_text(value)))
            });
            let display = |field: &Field| -> Option<String> {
                let value = nested_value(row, &field.name)?;
                if value.is_null() || matches!(&value, Value::String(text) if text.is_empty()) {
                    return None;
                }
                Some(format_cell_text(&value, field.format.as_deref(), options_for(field).as_deref()))
            };
            let avatar_el = avatar_field.and_then(|field| {
                let value = nested_value(row, &field.name)?;
                if value.is_null() || matches!(&value, Value::String(text) if text.is_empty()) {
                    return None;
                }
                Some(render(pattern(
                    "avatar",
                    [
                        ("name", Value::string(plain_text(&value))),
                        ("size", Value::string(if is_compact { "sm" } else { "md" })),
                        ("className", Value::string("flex-shrink-0")),
                    ],
                )))
            });
            let title_icon_box = (!is_compact)
                .then(|| title_field.and_then(|field| field.icon.clone()))
                .flatten()
                .map(|name| {
                    render(pattern(
                        "icon",
                        [
                            ("name", Value::string(name)),
                            ("size", Value::string("sm")),
                            ("className", Value::string("text-primary")),
                        ],
                    ))
                });
            let overlines: Vec<Element> = overline_fields
                .iter()
                .filter_map(|field| display(field))
                .map(|text| {
                    render(typography_spec("overline", None, "block text-primary mb-0.5", &text))
                })
                .collect();
            let compact_icon = is_compact
                .then(|| title_field.and_then(|field| field.icon.clone()))
                .flatten()
                .map(|name| {
                    render(pattern(
                        "icon",
                        [
                            ("name", Value::string(name)),
                            ("size", Value::string("xs")),
                            ("className", Value::string("text-primary flex-shrink-0")),
                        ],
                    ))
                });
            let title_el = title_text.as_ref().map(|text| {
                render(pattern(
                    "typography",
                    [
                        (
                            "variant",
                            Value::string(if is_compact {
                                "small"
                            } else if title_field.is_some_and(|field| field.variant.as_deref() == Some("h3")) {
                                "h3"
                            } else {
                                "h4"
                            }),
                        ),
                        ("weight", Value::string("semibold")),
                        (
                            "className",
                            Value::string(if is_compact { "truncate" } else { "line-clamp-2 break-words" }),
                        ),
                        ("content", Value::string(text.clone())),
                    ],
                ))
            });
            let badges: Vec<Element> = badge_fields
                .iter()
                .filter_map(|field| {
                    let value = nested_value(row, &field.name)?;
                    if value.is_null() || matches!(&value, Value::String(text) if text.is_empty()) {
                        return None;
                    }
                    let text = format_cell_text(&value, field.format.as_deref(), options_for(field).as_deref());
                    let icon = field.icon.as_ref().map(|name| {
                        render(pattern(
                            "icon",
                            [("name", Value::string(name.clone())), ("size", Value::string("xs"))],
                        ))
                    });
                    let badge = render(pattern(
                        "badge",
                        [
                            ("variant", Value::string(badge_variant(&plain_text(&value), field.color_map.as_ref()))),
                            ("label", Value::string(label_for(text, field.labels.as_ref()))),
                        ],
                    ));
                    Some(rsx! {
                        div { key: "{field.name}", class: "{HSTACK_XS} flex-shrink-0",
                            if let Some(icon) = icon { {icon} }
                            {badge}
                        }
                    })
                })
                .collect();
            let prose: Vec<Element> = prose_fields
                .iter()
                .filter_map(|field| display(field))
                .map(|text| {
                    let spec = typography_spec(
                        if is_compact { "small" } else { "body" },
                        Some("secondary"),
                        &cn(&["whitespace-pre-wrap break-words", if is_compact { "line-clamp-1" } else { "" }]),
                        &text,
                    );
                    let el = render(spec);
                    rsx! { div { class: if is_compact { "mt-0.5" } else { "mt-1" }, {el} } }
                })
                .collect();
            let body: Vec<Element> = body_fields
                .iter()
                .filter_map(|field| {
                    let value = nested_value(row, &field.name)?;
                    if value.is_null() || matches!(&value, Value::String(text) if text.is_empty()) {
                        return None;
                    }
                    let text = format_cell_text(&value, field.format.as_deref(), options_for(field).as_deref());
                    let icon = field.icon.as_ref().map(|name| {
                        render(pattern(
                            "icon",
                            [
                                ("name", Value::string(name.clone())),
                                ("size", Value::string("xs")),
                                ("className", Value::string("text-muted-foreground")),
                            ],
                        ))
                    });
                    let label = format!("{}:", field.label.clone().unwrap_or_else(|| field.name.clone()));
                    let label_el = render(typography_spec(
                        "caption",
                        Some("secondary"),
                        if field.format.as_deref() == Some("boolean") { "" } else { "sr-only" },
                        &label,
                    ));
                    let value_el = render(typography_spec("small", Some("secondary"), "", &text));
                    Some(rsx! {
                        div { key: "{field.name}", class: "{HSTACK_XS}",
                            if let Some(icon) = icon { {icon} }
                            {label_el}
                            {value_el}
                        }
                    })
                })
                .collect();
            let progress: Vec<Element> = progress_fields
                .iter()
                .filter_map(|field| {
                    let value = match nested_value(row, &field.name)? {
                        Value::Int(whole) => whole as f64,
                        Value::Number(float) => float,
                        _ => return None,
                    };
                    let icon = field.icon.as_ref().map(|name| {
                        render(pattern(
                            "icon",
                            [
                                ("name", Value::string(name.clone())),
                                ("size", Value::string("xs")),
                                ("className", Value::string("text-muted-foreground")),
                            ],
                        ))
                    });
                    let label = render(typography_spec(
                        "caption",
                        Some("secondary"),
                        "",
                        &field.label.clone().unwrap_or_else(|| field.name.clone()),
                    ));
                    let bar = render(pattern(
                        "progress-bar",
                        [("value", Value::Number(value)), ("max", Value::Int(100))],
                    ));
                    Some(rsx! {
                        div { key: "{field.name}", class: "mt-2 max-w-xs",
                            div { class: "{HSTACK_XS} mb-1",
                                if let Some(icon) = icon { {icon} }
                                {label}
                            }
                            {bar}
                        }
                    })
                })
                .collect();
            let actions_cluster = actions_el(ri, &row_actions[ri], false);
            let outer_class = cn(&["relative", if has_open { "cursor-pointer" } else { "" }, &enter]);
            let inner_class = cn(&[
                "group flex items-start gap-4 transition-all duration-fast",
                if is_compact { "px-4 py-2" } else { "px-6 py-4" },
                "hover:bg-muted/80",
                if !is_card && !is_compact {
                    "rounded-container border border-transparent hover:border-border"
                } else {
                    ""
                },
            ]);
            let meta_class = if is_compact {
                "flex flex-row gap-4 items-stretch justify-start flex-wrap mt-0.5"
            } else {
                "flex flex-row gap-4 items-stretch justify-start flex-wrap mt-1.5"
            };
            rsx! {
                div { key: "{id}", "data-entity-row": "", "data-entity-id": "{id}", "data-item-key": "{id}", class: "{outer_class}",
                    onclick: move |_| {
                        if let (Some(sink), Some((name, payload))) = (sink, &outer_click) {
                            sink.call((name.clone(), payload.clone()));
                        }
                    },
                    div { class: "{inner_class}",
                        if let Some(avatar_el) = avatar_el { {avatar_el} }
                        if let Some(icon) = title_icon_box {
                            div { class: "flex-shrink-0 w-10 h-10 rounded-container bg-primary/10 flex items-center justify-center", {icon} }
                        }
                        div { class: "flex-1 min-w-0",
                            for line in overlines.iter() { {line.clone()} }
                            div { class: "flex flex-row gap-2 items-center justify-start min-w-0",
                                if let Some(icon) = compact_icon { {icon} }
                                if let Some(title_el) = title_el {
                                    if has_open {
                                        div { role: "button", tabindex: "0", class: "min-w-0 flex-1 {STRETCHED_CONTROL}", {title_el} }
                                    } else {
                                        div { class: "min-w-0 flex-1", {title_el} }
                                    }
                                }
                                for badge in badges.iter() { {badge.clone()} }
                            }
                            for line in prose.iter() { {line.clone()} }
                            if !body.is_empty() {
                                div { class: "{meta_class}",
                                    for item in body.iter() { {item.clone()} }
                                }
                            }
                            for item in progress.iter() { {item.clone()} }
                        }
                        if let Some(actions_cluster) = actions_cluster { {actions_cluster} }
                    }
                    if divider { div { class: "{divider_class}" } }
                }
            }
        })
        .collect();

    let sentinel = (get_bool_or(props, "infiniteScroll", false)
        && get_string(props, "loadMoreEvent").is_some())
    .then(|| {
        render(pattern(
            "infinite-scroll-sentinel",
            [
                (
                    "loadMoreEvent",
                    Value::string(get_string_or(props, "loadMoreEvent", "")),
                ),
                ("hasMore", Value::Bool(get_bool_or(props, "hasMore", false))),
            ],
        ))
    });
    let root_class = cn(&[
        surface_class,
        if is_card { "" } else { "flex flex-col" },
        if !is_card && !is_compact {
            gap_class
        } else {
            ""
        },
        look_class,
        &class_name,
    ]);
    rsx! {
        div { class: "{root_class}", ..shared_root_attrs(props),
            for item in elements.iter() { {item.clone()} }
            if let Some(more) = more { div { class: "flex justify-center py-3", {more} } }
            if let Some(sentinel) = sentinel { {sentinel} }
        }
    }
}

fn relation_cell_text(value: &Value, options: Option<&[Value]>) -> Option<String> {
    crate::renderer::relation_cell_display(value, options)
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
        rsx! { DataListBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
