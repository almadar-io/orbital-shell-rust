use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::merge_class_name;
use crate::renderer::{
    array_value, cn, format_cell_text, format_display_value, logged_row_payload, logged_row_route,
    nested_value, object_value, pattern_spec as pattern, plain_text, relation_cell_display,
    typography_spec, DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `table-view` pattern (web: TableView.tsx).
pub struct TableViewRenderer;

impl TableViewRenderer {
    const NAME: &str = "table-view";
}

const STRETCHED_CONTROL: &str = "after:absolute after:inset-0 after:content-[''] after:rounded-[inherit] focus-visible:outline-none focus-visible:after:ring-2 focus-visible:after:ring-primary focus-visible:after:ring-inset";
const STRETCHED_ABOVE: &str = "relative z-[1]";

struct Column {
    key: String,
    field: String,
    label: String,
    width: Option<String>,
    align: String,
    class_name: String,
    weight: String,
    format: Option<String>,
    color_map: Option<Value>,
    labels: Option<Value>,
    variant: Option<String>,
    icon: Option<String>,
    sortable: bool,
}

fn columns_of(props: &Value) -> Vec<Column> {
    let defs = ["columns", "fields"]
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
            let key = get_string(def, "key").or_else(|| get_string(def, "name"))?;
            Some(Column {
                field: get_string(def, "field").unwrap_or_else(|| key.clone()),
                label: get_string(def, "header")
                    .or_else(|| get_string(def, "label"))
                    .unwrap_or_else(|| key.clone()),
                key,
                width: get_string(def, "width"),
                align: get_string_or(def, "align", "left"),
                class_name: get_string_or(def, "className", ""),
                weight: get_string_or(def, "weight", "normal"),
                format: get_string(def, "format"),
                color_map: def.get("colorMap").cloned(),
                labels: def.get("labels").cloned(),
                variant: get_string(def, "variant"),
                icon: get_string(def, "icon"),
                sortable: get_bool_or(def, "sortable", false),
            })
        })
        .collect()
}

fn align_class(align: &str) -> &'static str {
    match align {
        "center" => "@sm/table:justify-center @sm/table:text-center",
        "right" => "@sm/table:justify-end @sm/table:text-right",
        _ => "@sm/table:justify-start @sm/table:text-left",
    }
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

#[derive(Props, Clone, PartialEq)]
struct TableViewBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn TableViewBody(p: TableViewBodyProps) -> Element {
    let props = &p.props;
    let registry = &p.registry;
    let page_size = get_number_or(props, "pageSize", 0.0).max(0.0) as usize;
    let mut visible = use_signal(|| if page_size > 0 { page_size } else { usize::MAX });
    let mut local_selected = use_signal(Vec::<String>::new);
    let base = p.view.as_view();
    let sink = base.sink;

    let columns = columns_of(props);
    let actions = get_array(props, "itemActions");
    let has_actions = !actions.is_empty();
    let render_item = props.get("renderItem").and_then(render_item_lambda);
    let has_render_prop = render_item.is_some();
    let look = get_string_or(props, "look", "dense");
    let (row_pad, head_pad, striped, divider) = match look.as_str() {
        "spacious" => (
            "px-card-lg py-card-md",
            "px-card-lg py-card-sm",
            false,
            true,
        ),
        "striped" => (
            "px-card-md py-card-sm",
            "px-card-md py-card-sm",
            true,
            false,
        ),
        "borderless" => (
            "px-card-md py-card-sm",
            "px-card-md py-card-sm",
            false,
            false,
        ),
        "bordered" => (
            "px-card-md py-card-sm",
            "px-card-md py-card-sm",
            false,
            true,
        ),
        _ => ("px-card-md py-2", "px-card-md py-2", false, true),
    };
    let id_field = get_string_or(props, "dndItemIdField", "id");
    let selectable = get_bool_or(props, "selectable", false);
    let select_event = get_string(props, "selectEvent");
    let sort_event = get_string(props, "sortEvent");
    let sort_column = get_string(props, "sortColumn");
    let sort_direction = get_string(props, "sortDirection");
    let item_click = get_string(props, "itemClickEvent");

    let all_rows = get_entity_rows_from(props, base.entities);
    let total = all_rows.len();
    let shown = if page_size > 0 {
        visible().min(total)
    } else {
        total
    };
    let rows: Vec<Value> = all_rows.into_iter().take(shown).collect();
    let has_more = page_size > 0 && shown < total;
    let ids: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            row.get(&id_field)
                .filter(|value| !value.is_null())
                .map(plain_text)
                .unwrap_or_else(|| index.to_string())
        })
        .collect();

    let controlled_selection = props
        .get("selectedIds")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(plain_text).collect::<Vec<_>>());
    let selected: Vec<String> = controlled_selection
        .clone()
        .unwrap_or_else(|| local_selected());
    let all_selected = selectable && !rows.is_empty() && ids.iter().all(|id| selected.contains(id));

    let row_actions: Vec<Vec<usize>> = rows
        .iter()
        .map(|row| {
            actions
                .iter()
                .enumerate()
                .filter(|(_, action)| row_action_is_shown(action, row, None))
                .filter(|(_, action)| logged_row_payload(action, row).is_some())
                .map(|(index, _)| index)
                .collect()
        })
        .collect();

    let relay: EventSink = {
        let rows = rows.clone();
        let actions = actions.clone();
        let ids = ids.clone();
        let selected = selected.clone();
        let select_event = select_event.clone();
        let controlled = controlled_selection.is_some();
        Callback::new(move |(event, payload): (String, Value)| {
            let mut emit_selection = |next: Vec<String>| {
                if !controlled {
                    local_selected.set(next.clone());
                }
                if let (Some(sink), Some(select_event)) = (sink, &select_event) {
                    let ids = Value::Array(
                        next.into_iter()
                            .map(Value::string)
                            .collect::<Vec<_>>()
                            .into(),
                    );
                    sink.call((select_event.clone(), object_value([("selectedIds", ids)])));
                }
            };
            if event == "__tv_more" {
                let step = if page_size > 0 { page_size } else { 5 };
                visible.set(visible().saturating_add(step));
            } else if event == "__tv_all" {
                if all_selected {
                    emit_selection(Vec::new());
                } else {
                    emit_selection(ids.clone());
                }
            } else if let Some(id) = event.strip_prefix("__tv_row:") {
                let mut next = selected.clone();
                if let Some(position) = next.iter().position(|item| item == id) {
                    next.remove(position);
                } else {
                    next.push(id.to_string());
                }
                emit_selection(next);
            } else if let Some(target) = event.strip_prefix("__tv_act:") {
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

    let floors: Vec<usize> = columns
        .iter()
        .map(|col| {
            let options = relation_options(props, &col.field);
            let longest = rows.iter().fold(col.label.chars().count(), |widest, row| {
                let text = nested_value(row, &col.field)
                    .map(|value| {
                        format_cell_text(&value, col.format.as_deref(), options.as_deref())
                    })
                    .unwrap_or_default();
                widest.max(text.chars().count())
            });
            let chrome = if col.format.as_deref() == Some("badge") {
                4
            } else {
                0
            };
            (longest + chrome).min(32)
        })
        .collect();
    let mut tracks: Vec<String> = Vec::new();
    if selectable {
        tracks.push("auto".to_string());
    }
    for (index, col) in columns.iter().enumerate() {
        tracks.push(
            col.width
                .clone()
                .unwrap_or_else(|| format!("minmax({}ch, 1fr)", floors[index])),
        );
    }
    if has_actions {
        tracks.push("3rem".to_string());
    }
    let grid = format!("grid-template-columns:{}", tracks.join(" "));

    let status: Option<Element> = if get_bool_or(props, "isLoading", false) {
        let spec = props
            .get("skeleton")
            .cloned()
            .unwrap_or_else(|| Value::string("table"));
        Some(render(pattern("skeleton", [("spec", spec)])))
    } else if let Some(message) = props
        .get("error")
        .and_then(|error| get_string(error, "message"))
    {
        let text = render(typography_spec("body", Some("error"), "", &message));
        Some(rsx! { div { class: "text-center py-8", {text} } })
    } else if rows.is_empty() {
        let mut fields = vec![(
            "title",
            Value::string(
                get_string(props, "emptyTitle")
                    .filter(|text| !text.is_empty())
                    .or_else(|| get_string(props, "emptyMessage").filter(|text| !text.is_empty()))
                    .unwrap_or_else(|| crate::i18n::t("empty.noItems")),
            ),
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
        Some(render(pattern("empty-state", fields)))
    } else {
        None
    };

    let show_header = !columns.is_empty() || has_render_prop;
    let header = show_header.then(|| {
        let header_class = cn(&[
            "rounded-none shadow-none",
            "hidden @sm/table:grid items-center gap-3 sticky top-0 z-10",
            "bg-[var(--color-surface-subtle)] border-b border-[var(--color-border)]",
            "text-muted-foreground uppercase font-semibold tracking-wide",
            head_pad,
        ]);
        let select_all = selectable.then(|| {
            render(pattern(
                "checkbox",
                [
                    ("checked", Value::Bool(all_selected)),
                    ("onChange", Value::string("__tv_all")),
                    ("aria-label", Value::string(crate::i18n::t("aria.selectAllRows"))),
                ],
            ))
        });
        let cells: Vec<Element> = columns
            .iter()
            .map(|col| {
                let active = sort_column.as_deref() == Some(col.field.as_str());
                let sortable = col.sortable && sort_event.is_some();
                let inner_class = cn(&[
                    "bg-transparent rounded-none shadow-none flex items-center gap-1 min-w-0 text-xs",
                    if sortable {
                        "cursor-pointer select-none hover:text-foreground"
                    } else {
                        ""
                    },
                ]);
                let direction = if active && sort_direction.as_deref() == Some("asc") {
                    "desc"
                } else {
                    "asc"
                };
                let payload = object_value([
                    ("column", Value::string(col.field.clone())),
                    ("direction", Value::string(direction)),
                ]);
                let event = sort_event.clone().filter(|_| sortable);
                let icon = col.icon.as_ref().map(|name| {
                    render(pattern(
                        "icon",
                        [("name", Value::string(name.clone())), ("size", Value::string("xs"))],
                    ))
                });
                let sort_icon = sortable.then(|| {
                    let name = if active {
                        if sort_direction.as_deref() == Some("asc") {
                            "chevron-up"
                        } else {
                            "chevron-down"
                        }
                    } else {
                        "chevrons-up-down"
                    };
                    render(pattern(
                        "icon",
                        [
                            ("name", Value::string(name)),
                            ("size", Value::string("xs")),
                            (
                                "className",
                                Value::string(if active {
                                    "flex-shrink-0 text-foreground"
                                } else {
                                    "flex-shrink-0 opacity-40"
                                }),
                            ),
                        ],
                    ))
                });
                let aria_sort = (sortable).then(|| {
                    if active {
                        if sort_direction.as_deref() == Some("asc") {
                            "ascending"
                        } else {
                            "descending"
                        }
                    } else {
                        "none"
                    }
                });
                rsx! {
                    div { key: "{col.key}", role: "columnheader", "aria-sort": aria_sort,
                        class: cn(&["bg-transparent rounded-none shadow-none flex items-center min-w-0", align_class(&col.align)]),
                        div { class: "{inner_class}",
                            role: sortable.then_some("button"), tabindex: sortable.then_some("0"),
                            onclick: move |_| {
                                if let (Some(sink), Some(event)) = (sink, &event) {
                                    sink.call((event.clone(), payload.clone()));
                                }
                            },
                            if let Some(icon) = icon { {icon} }
                            span { class: "truncate", "{col.label}" }
                            if let Some(sort_icon) = sort_icon { {sort_icon} }
                        }
                    }
                }
            })
            .collect();
        rsx! {
            div { role: "row", class: "{header_class}", style: "{grid}",
                if let Some(select_all) = select_all {
                    div { role: "columnheader", class: "bg-transparent rounded-none shadow-none flex items-center", {select_all} }
                }
                for cell in cells.iter() { {cell.clone()} }
                if has_actions {
                    div { "aria-hidden": "true", class: "rounded-none shadow-none sticky right-0 bg-[var(--color-surface-subtle)] border-l border-[var(--color-border)] h-full" }
                }
            }
        }
    });

    let body: Option<Element> = if status.is_some() {
        None
    } else {
        let row_elements: Vec<Element> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let id = ids[index].clone();
                let open_payload = item_click
                    .as_ref()
                    .and_then(|_| logged_row_payload(&object_value::<&str>([]), row));
                let open: Option<(String, Value)> = item_click.clone().zip(open_payload);
                let row_event = open.clone();
                let row_class = cn(&[
                    "bg-transparent rounded-none shadow-none group relative transition-colors duration-fast",
                    if has_render_prop {
                        "flex items-center gap-3"
                    } else {
                        "flex flex-col gap-1 @sm/table:grid @sm/table:items-center @sm/table:gap-3"
                    },
                    if open.is_some() { "cursor-pointer" } else { "" },
                    row_pad,
                    if divider { "border-b border-[var(--color-border)]" } else { "" },
                    if striped && index % 2 == 1 { "bg-[var(--color-surface-subtle)]" } else { "" },
                    "hover:bg-[var(--color-surface-subtle)]",
                    if look == "bordered" {
                        "@sm/table:[&>*]:border-r @sm/table:[&>*]:border-[var(--color-border)] @sm/table:[&>*:last-child]:border-r-0"
                    } else {
                        ""
                    },
                ]);
                let row_style = if has_render_prop { String::new() } else { grid.clone() };
                let checkbox = selectable.then(|| {
                    render(pattern(
                        "checkbox",
                        [
                            ("checked", Value::Bool(selected.contains(&id))),
                            ("onChange", Value::string(format!("__tv_row:{id}"))),
                            ("aria-label", Value::string(format!("Select row {id}"))),
                        ],
                    ))
                });
                let check_class = cn(&["bg-transparent rounded-none shadow-none flex items-center", STRETCHED_ABOVE]);
                let stop_check = open.is_some();
                let cells: Vec<Element> = if let Some(lambda) = &render_item {
                    let custom = render(lambda.apply(row, index));
                    let overlay = open.clone();
                    let sink_open = open.clone();
                    vec![rsx! {
                        div { role: "cell", class: "bg-transparent rounded-none shadow-none relative flex-1 min-w-0",
                            if overlay.is_some() {
                                div { role: "button", tabindex: "0", "aria-label": format!("Open item {}", index + 1),
                                    class: "absolute inset-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset",
                                    onclick: move |event| {
                                        event.stop_propagation();
                                        if let (Some(sink), Some((name, payload))) = (sink, &sink_open) {
                                            sink.call((name.clone(), payload.clone()));
                                        }
                                    },
                                }
                            }
                            div { class: if open.is_some() { "relative" } else { "" }, {custom} }
                        }
                    }]
                } else {
                    let open_column = columns
                        .iter()
                        .find(|col| matches!(col.variant.as_deref(), Some("h3" | "h4")))
                        .or(columns.first())
                        .map(|col| col.key.clone());
                    columns
                        .iter()
                        .map(|col| {
                            let raw = nested_value(row, &col.field);
                            let options = relation_options(props, &col.field);
                            let opens = open.is_some() && open_column.as_deref() == Some(col.key.as_str());
                            let is_title = matches!(col.variant.as_deref(), Some("h3" | "h4"));
                            let weight = match col.weight.as_str() {
                                "medium" => "font-medium",
                                "semibold" => "font-semibold",
                                _ => "",
                            };
                            let cell_class = cn(&[
                                "bg-transparent rounded-none shadow-none flex items-center min-w-0 gap-3",
                                if is_title {
                                    "font-medium @sm/table:font-[inherit]"
                                } else {
                                    "justify-between"
                                },
                                if is_title && has_actions { "pr-10 @sm/table:pr-0" } else { "" },
                                align_class(&col.align),
                                weight,
                                &col.class_name,
                            ]);
                            let badge = col.format.as_deref() == Some("badge")
                                && raw.as_ref().is_some_and(|value| {
                                    !value.is_null() && !matches!(value, Value::String(text) if text.is_empty())
                                });
                            let content: Element = match (&raw, badge) {
                                (Some(value), true) => {
                                    let key = plain_text(value);
                                    let label = relation_cell_display(value, options.as_deref())
                                        .or_else(|| col.labels.as_ref().and_then(|labels| get_string(labels, &key)))
                                        .unwrap_or_else(|| match value {
                                            Value::Bool(_) => format_display_value(value, Some("boolean")),
                                            _ => key.clone(),
                                        });
                                    render(pattern(
                                        "badge",
                                        [
                                            ("variant", Value::string(badge_variant(&key, col.color_map.as_ref()))),
                                            ("size", Value::string("sm")),
                                            ("className", Value::string("whitespace-nowrap")),
                                            ("label", Value::string(label)),
                                        ],
                                    ))
                                }
                                _ => {
                                    let text = raw
                                        .as_ref()
                                        .map(|value| format_cell_text(value, col.format.as_deref(), options.as_deref()))
                                        .unwrap_or_default();
                                    rsx! { span { class: "truncate text-foreground", "{text}" } }
                                }
                            };
                            let open_for_cell = open.clone();
                            rsx! {
                                div { key: "{col.key}", role: "cell", class: "{cell_class}",
                                    if !is_title {
                                        span { class: "@sm/table:hidden shrink-0 text-xs text-muted-foreground", "data-stacked-label": "", "{col.label}" }
                                    }
                                    if opens {
                                        div { role: "button", tabindex: "0", class: "bg-transparent rounded-none shadow-none min-w-0 {STRETCHED_CONTROL}",
                                            onclick: move |event| {
                                                event.stop_propagation();
                                                if let (Some(sink), Some((name, payload))) = (sink, &open_for_cell) {
                                                    sink.call((name.clone(), payload.clone()));
                                                }
                                            },
                                            {content}
                                        }
                                    } else { {content} }
                                }
                            }
                        })
                        .collect()
                };
                let menu = (has_actions && !row_actions[index].is_empty()).then(|| {
                    let items: Vec<Value> = row_actions[index]
                        .iter()
                        .map(|&action_index| {
                            let action = &actions[action_index];
                            let route = logged_row_route(action, row);
                            let mut fields = vec![
                                ("id", Value::string(action_index.to_string())),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                                (
                                    "variant",
                                    Value::string(if get_string(action, "variant").as_deref() == Some("danger") {
                                        "danger"
                                    } else {
                                        "default"
                                    }),
                                ),
                            ];
                            if let Some(icon) = action.get("icon") {
                                fields.push(("icon", icon.clone()));
                            }
                            match route {
                                Some(route) => fields.push(("href", Value::string(route))),
                                None => fields.push((
                                    "event",
                                    Value::string(format!("__tv_act:{index}:{action_index}")),
                                )),
                            }
                            object_value(fields)
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
                                        ("aria-label", Value::string(crate::i18n::t("common.actions"))),
                                    ],
                                ),
                            ),
                            ("items", array_value(items)),
                        ],
                    ))
                });
                let actions_class = cn(&[
                    "flex flex-row gap-1 items-stretch justify-end flex-shrink-0 z-[1] transition-colors",
                    "absolute top-1 right-1 @sm/table:sticky @sm/table:top-auto @sm/table:right-0",
                    "@sm/table:border-l @sm/table:border-[var(--color-border)]",
                    if striped && index % 2 == 1 {
                        "bg-[var(--color-surface-subtle)]"
                    } else {
                        "bg-[var(--color-card)] group-hover:bg-[var(--color-surface-subtle)]"
                    },
                ]);
                rsx! {
                    div { key: "{id}", role: "row", "data-entity-row": "", "data-entity-id": "{id}",
                        "aria-selected": selectable.then(|| selected.contains(&id).to_string()),
                        class: "{row_class}", style: "{row_style}",
                        onclick: move |_| {
                            if let (Some(sink), Some((name, payload))) = (sink, &row_event) {
                                sink.call((name.clone(), payload.clone()));
                            }
                        },
                        if let Some(checkbox) = checkbox {
                            div { role: "cell", class: "{check_class}",
                                onclick: move |event| { if stop_check { event.stop_propagation(); } },
                                {checkbox}
                            }
                        }
                        for cell in cells.iter() { {cell.clone()} }
                        if has_actions {
                            div { role: "cell", class: "{actions_class}",
                                onclick: move |event| { if stop_check { event.stop_propagation(); } },
                                if let Some(menu) = menu { {menu} }
                            }
                        }
                    }
                }
            })
            .collect();
        Some(rsx! {
            div { role: "rowgroup", class: "bg-transparent rounded-none shadow-none",
                for row in row_elements.iter() { {row.clone()} }
            }
        })
    };

    let more = (status.is_none() && has_more).then(|| {
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
                ("action", Value::string("__tv_more")),
            ],
        ))
    });
    let surface_class = if get_string_or(props, "surface", "auto") == "auto" {
        "surface-content overflow-hidden"
    } else {
        ""
    };
    let root_class = merge_class_name(
        &cn(&[
            "bg-transparent rounded-none shadow-none @container/table w-full text-sm",
            surface_class,
        ]),
        props,
    );

    rsx! {
        div { role: "table", class: "{root_class}", ..shared_root_attrs(props),
            if let Some(header) = header { {header} }
            if let Some(status) = status {
                div { role: "rowgroup", class: "bg-transparent rounded-none shadow-none", div { role: "row", class: "bg-transparent rounded-none shadow-none", div { role: "cell", class: "bg-transparent rounded-none shadow-none w-full", {status} } } }
            } else if let Some(body) = body { {body} }
            if let Some(more) = more { div { class: "flex justify-center py-3", {more} } }
        }
    }
}

impl DioxusPatternRenderer for TableViewRenderer {
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
        rsx! { TableViewBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
