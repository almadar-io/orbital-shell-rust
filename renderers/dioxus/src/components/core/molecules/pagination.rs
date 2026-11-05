use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_number, get_number_or, get_string, merge_class_name,
};
use orbital_core::{Value, ValueMap};

pub struct PaginationRenderer;

fn page_emissions(
    declarative: &Option<String>,
    callback: &Option<String>,
    page: u32,
) -> Vec<(String, Value)> {
    let mut emissions = Vec::new();
    if let Some(name) = declarative {
        let mut payload = ValueMap::new();
        payload.insert("page".into(), Value::Int(page as i64));
        emissions.push((name.clone(), Value::Object(payload.into())));
    }
    if let Some(name) = callback {
        if Some(name) != declarative.as_ref() {
            emissions.push((name.clone(), Value::Int(page as i64)));
        }
    }
    emissions
}

fn emit_page(
    sink: Option<EventSink>,
    declarative: &Option<String>,
    callback: &Option<String>,
    page: u32,
) {
    if let Some(sink) = sink {
        for event in page_emissions(declarative, callback, page) {
            sink.call(event);
        }
    }
}

fn emit_page_size(
    sink: Option<EventSink>,
    declarative: &Option<String>,
    callback: &Option<String>,
    size: u32,
) {
    if let Some(sink) = sink {
        if let Some(name) = declarative {
            let mut payload = ValueMap::new();
            payload.insert("pageSize".into(), Value::Int(size as i64));
            sink.call((name.clone(), Value::Object(payload.into())));
        }
        if let Some(name) = callback {
            if Some(name) != declarative.as_ref() {
                sink.call((name.clone(), Value::Int(size as i64)));
            }
        }
    }
}

#[component]
fn PaginationFrame(
    current_page: u32,
    total_pages: u32,
    total_items: Option<u32>,
    show_total: bool,
    show_page_size: bool,
    page_size: Option<u32>,
    page_size_options: Vec<u32>,
    show_jump: bool,
    page_event: Option<String>,
    page_callback: Option<String>,
    size_event: Option<String>,
    size_callback: Option<String>,
    sink: Option<EventSink>,
    class: String,
) -> Element {
    let mut jump = use_signal(String::new);
    let previous_disabled = current_page <= 1 || total_pages == 0;
    let next_disabled = current_page >= total_pages;
    let previous_event = page_event.clone();
    let previous_callback = page_callback.clone();
    let next_event = page_event.clone();
    let next_callback = page_callback.clone();
    let jump_event = page_event.clone();
    let jump_callback = page_callback.clone();
    let jump_key_event = page_event.clone();
    let jump_key_callback = page_callback.clone();
    let page_size_event = size_event.clone();
    let page_size_callback = size_callback.clone();
    let can_choose_size =
        show_page_size && page_size.is_some() && (size_event.is_some() || size_callback.is_some());

    rsx! {
        nav { class: "{class}", aria_label: "Pagination",
            div { class: "flex flex-wrap items-center justify-between gap-x-4 gap-y-2",
                div { class: "flex flex-wrap items-center gap-2",
                    if show_total {
                        if let Some(total) = total_items {
                            span { class: "text-sm text-muted-foreground", "Total {total}" }
                        }
                    }
                    if can_choose_size {
                        span { class: "text-sm text-muted-foreground", "Show" }
                        dioxus_primitives::select::Select::<String> {
                            default_value: page_size.map(|size| size.to_string()),
                            on_value_change: move |value: Option<String>| {
                                if let Some(size) = value.and_then(|value| value.parse::<u32>().ok()) {
                                    emit_page_size(sink, &page_size_event, &page_size_callback, size);
                                }
                            },
                            dioxus_primitives::select::SelectTrigger {
                                aria_label: "Page size",
                                class: "rounded-interactive border border-border bg-card px-2 py-1 text-sm",
                                dioxus_primitives::select::SelectValue { placeholder: "Size" }
                            }
                            dioxus_primitives::select::SelectList {
                                class: "absolute z-50 bg-card border border-border rounded-interactive shadow-elevation-popover",
                                for (index, size) in page_size_options.iter().enumerate() {
                                    dioxus_primitives::select::SelectOption::<String> {
                                        key: "{size}", index: index, value: size.to_string(), "{size}"
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "flex items-center gap-1",
                    button {
                        class: "inline-flex items-center gap-1 rounded-interactive px-3 h-button-sm text-sm bg-secondary text-secondary-foreground disabled:opacity-50",
                        disabled: previous_disabled, aria_label: "Previous page",
                        onclick: move |_| {
                            if !previous_disabled {
                                emit_page(sink, &previous_event, &previous_callback, current_page - 1);
                            }
                        },
                        "‹ Previous"
                    }
                    span { class: "px-2 text-sm text-muted-foreground tabular-nums", "Page {current_page} of {total_pages}" }
                    button {
                        class: "inline-flex items-center gap-1 rounded-interactive px-3 h-button-sm text-sm bg-secondary text-secondary-foreground disabled:opacity-50",
                        disabled: next_disabled, aria_label: "Next page",
                        onclick: move |_| {
                            if !next_disabled {
                                emit_page(sink, &next_event, &next_callback, current_page + 1);
                            }
                        },
                        "Next ›"
                    }
                }
                if show_jump {
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm text-muted-foreground", "Go to" }
                        input { class: "w-20 rounded-interactive border border-border bg-card px-2 py-1 text-sm", r#type: "number",
                            min: "1", max: "{total_pages}", value: "{jump}", placeholder: "Page", aria_label: "Jump to page",
                            oninput: move |event| jump.set(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Enter {
                                    let entered = jump.read().clone();
                                    if let Ok(page) = entered.parse::<u32>() {
                                        if (1..=total_pages).contains(&page) {
                                            emit_page(sink, &jump_key_event, &jump_key_callback, page);
                                            jump.set(String::new());
                                        }
                                    }
                                }
                            }
                        }
                        button { class: "rounded-interactive px-3 h-button-sm text-sm hover:bg-muted",
                            onclick: move |_| {
                                let entered = jump.read().clone();
                                if let Ok(page) = entered.parse::<u32>() {
                                    if (1..=total_pages).contains(&page) {
                                        emit_page(sink, &jump_event, &jump_callback, page);
                                        jump.set(String::new());
                                    }
                                }
                            },
                            "Go"
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for PaginationRenderer {
    fn pattern_name(&self) -> &str {
        "pagination"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let current_page = get_number_or(props, "currentPage", 1.0).max(0.0) as u32;
        let total_pages = get_number_or(props, "totalPages", 0.0).max(0.0) as u32;
        let page_size_options = {
            let values = get_array(props, "pageSizeOptions");
            if values.is_empty() {
                vec![10, 20, 50, 100]
            } else {
                values
                    .iter()
                    .filter_map(Value::as_int)
                    .filter_map(|size| u32::try_from(size).ok())
                    .collect()
            }
        };
        let class = merge_class_name("@container w-full", props);
        let attrs = shared_root_attrs(props);
        rsx! { div { ..attrs,
            PaginationFrame {
                current_page, total_pages,
                total_items: get_number(props, "totalItems").map(|n| n.max(0.0) as u32),
                show_total: get_bool_or(props, "showTotal", false),
                show_page_size: get_bool_or(props, "showPageSize", false),
                page_size: get_number(props, "pageSize").map(|n| n.max(0.0) as u32),
                page_size_options,
                show_jump: get_bool_or(props, "showJumpToPage", false),
                page_event: get_string(props, "pageChangeEvent"),
                page_callback: get_string(props, "onPageChange"),
                size_event: get_string(props, "pageSizeChangeEvent"),
                size_callback: get_string(props, "onPageSizeChange"),
                sink: view.sink,
                class,
            }
        } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_event_payload_is_named_and_callback_keeps_scalar() {
        let seen = page_emissions(&Some("PAGE".into()), &Some("CALLBACK".into()), 3);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, "PAGE");
        assert_eq!(
            seen[0]
                .1
                .as_object()
                .and_then(|map| map.get("page"))
                .and_then(Value::as_int),
            Some(3)
        );
        assert_eq!(seen[1], ("CALLBACK".into(), Value::Int(3)));
        assert_eq!(
            page_emissions(&Some("PAGE".into()), &Some("PAGE".into()), 3).len(),
            1
        );
    }
}
