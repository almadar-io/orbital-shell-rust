use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, merge_class_name, object_value, pattern_spec};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_number, get_number_or, get_string,
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

#[derive(Clone, Copy, PartialEq)]
enum PageItem {
    Page(i64),
    Ellipsis,
}

fn page_numbers(current: i64, total: i64, max_visible: i64) -> Vec<PageItem> {
    let mut pages = Vec::new();
    let half = max_visible / 2;
    if total <= max_visible {
        for page in 1..=total {
            pages.push(PageItem::Page(page));
        }
    } else if current <= half + 1 {
        for page in 1..=(max_visible - 2) {
            pages.push(PageItem::Page(page));
        }
        pages.push(PageItem::Ellipsis);
        pages.push(PageItem::Page(total));
    } else if current >= total - half {
        pages.push(PageItem::Page(1));
        pages.push(PageItem::Ellipsis);
        for page in (total - max_visible + 3)..=total {
            pages.push(PageItem::Page(page));
        }
    } else {
        pages.push(PageItem::Page(1));
        pages.push(PageItem::Ellipsis);
        for page in (current - half + 2)..=(current + half - 2) {
            pages.push(PageItem::Page(page));
        }
        pages.push(PageItem::Ellipsis);
        pages.push(PageItem::Page(total));
    }
    pages
}

fn hstack_class(gap: &str, extra: &str) -> String {
    cn(&["flex flex-row", gap, "items-center justify-start", extra])
}

fn small_text(content: &str, color: &str, class: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string("small")),
            ("color", Value::string(color)),
            ("className", Value::string(class)),
        ],
    )
}

fn hidden_label(text: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("as", Value::string("span")),
            ("content", Value::string(text)),
            ("variant", Value::string("small")),
            ("color", Value::string("inherit")),
            ("weight", Value::string("medium")),
            ("className", Value::string("hidden @md:inline")),
        ],
    )
}

struct PageAction {
    event: Option<String>,
    callback: Option<String>,
}

impl PageAction {
    fn fields(&self, page: i64) -> Vec<(&'static str, Value)> {
        match (&self.event, &self.callback) {
            (Some(name), _) => vec![
                ("action", Value::string(name.to_string())),
                ("actionPayload", object_value([("page", Value::Int(page))])),
            ],
            (None, Some(name)) => vec![
                ("action", Value::string(name.to_string())),
                ("actionPayload", Value::Int(page)),
            ],
            (None, None) => Vec::new(),
        }
    }
}

struct NavButton<'a> {
    variant: &'a str,
    label: Option<String>,
    icon: Option<(&'static str, &'static str)>,
    aria_label: Option<&'a str>,
    disabled: bool,
    class: Option<&'a str>,
    current: bool,
    action: Vec<(&'static str, Value)>,
    child: Option<Value>,
}

impl NavButton<'_> {
    fn spec(self) -> Value {
        let mut fields: Vec<(&'static str, Value)> = vec![
            ("variant", Value::string(self.variant)),
            ("size", Value::string("sm")),
        ];
        if self.disabled {
            fields.push(("disabled", Value::Bool(true)));
        }
        if let Some((key, name)) = self.icon {
            fields.push((key, Value::string(name)));
        }
        if let Some(aria) = self.aria_label {
            fields.push(("aria-label", Value::string(aria)));
        }
        if self.current {
            fields.push(("aria-current", Value::string("page")));
        }
        if let Some(class) = self.class {
            fields.push(("className", Value::string(class)));
        }
        fields.extend(self.action);
        if let Some(label) = self.label {
            fields.push(("label", Value::string(label)));
        }
        if let Some(child) = self.child {
            fields.push(("children", Value::Array(vec![child].into())));
        }
        pattern_spec("button", fields)
    }
}

#[derive(Props, Clone, PartialEq)]
struct PaginationProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn PaginationFrame(input: PaginationProps) -> Element {
    let mut jump = use_signal(String::new);
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let sink = view.sink;

    let current = get_number_or(props, "currentPage", 1.0) as i64;
    let total = get_number_or(props, "totalPages", 0.0) as i64;
    let max_visible = get_number_or(props, "maxVisiblePages", 7.0) as i64;
    let show_total = get_bool_or(props, "showTotal", false);
    let total_items = get_number(props, "totalItems").map(|n| n as i64);
    let show_jump = get_bool_or(props, "showJumpToPage", false);
    let page_size = get_number(props, "pageSize")
        .filter(|n| *n != 0.0)
        .map(|n| n as u32);
    let size_options: Vec<u32> = {
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
    let page_event = get_string(props, "pageChangeEvent");
    let page_callback = get_string(props, "onPageChange");
    let size_event = get_string(props, "pageSizeChangeEvent");
    let size_callback = get_string(props, "onPageSizeChange");
    let can_choose_size = get_bool_or(props, "showPageSize", false)
        && page_size.is_some()
        && (size_event.is_some() || size_callback.is_some());
    let action = PageAction {
        event: page_event.clone(),
        callback: page_callback.clone(),
    };

    let nav_class = merge_class_name(
        "bg-transparent rounded-none shadow-none @container w-full",
        props,
    );
    let row_class = cn(&[
        "flex flex-row gap-4 items-center justify-start",
        "flex-wrap justify-between gap-x-4 gap-y-2",
    ]);
    let left_class = hstack_class("gap-2", "flex-wrap");
    let group_class = hstack_class("gap-2", "");
    let nav_group_class = hstack_class("gap-1", "");
    let pages_class = hstack_class("gap-1", "hidden @md:flex");

    let total_spec = if show_total {
        total_items.map(|items| small_text(&format!("Total: {items}"), "secondary", ""))
    } else {
        None
    };
    let show_spec = small_text("Show:", "secondary", "");
    let previous = NavButton {
        variant: "secondary",
        label: None,
        icon: Some(("icon", "chevron-left")),
        aria_label: Some("Previous"),
        disabled: current == 1,
        class: None,
        current: false,
        action: action.fields(current - 1),
        child: Some(hidden_label("Previous")),
    }
    .spec();
    let next = NavButton {
        variant: "secondary",
        label: None,
        icon: Some(("iconRight", "chevron-right")),
        aria_label: Some("Next"),
        disabled: current == total,
        class: None,
        current: false,
        action: action.fields(current + 1),
        child: Some(hidden_label("Next")),
    }
    .spec();
    let page_of = small_text(
        &format!("{current} / {total}"),
        "secondary",
        "px-2 tabular-nums @md:hidden",
    );
    let page_specs: Vec<Value> = page_numbers(current, total, max_visible)
        .iter()
        .map(|item| match item {
            PageItem::Ellipsis => pattern_spec(
                "typography",
                [
                    ("content", Value::string("…")),
                    ("variant", Value::string("small")),
                    ("color", Value::string("muted")),
                    ("aria-hidden", Value::string("true")),
                    ("className", Value::string("px-2")),
                ],
            ),
            PageItem::Page(page) => NavButton {
                variant: if *page == current { "primary" } else { "ghost" },
                label: Some(page.to_string()),
                icon: None,
                aria_label: None,
                disabled: false,
                class: Some("min-w-[2.5rem] tabular-nums"),
                current: *page == current,
                action: action.fields(*page),
                child: None,
            }
            .spec(),
        })
        .collect();
    let go_to = small_text("Go to:", "secondary", "");
    let go_button = NavButton {
        variant: "ghost",
        label: Some("Go".to_string()),
        icon: None,
        aria_label: None,
        disabled: false,
        class: None,
        current: false,
        action: Vec::new(),
        child: None,
    }
    .spec();

    let click_event = page_event.clone();
    let click_callback = page_callback.clone();
    let key_event = page_event.clone();
    let key_callback = page_callback.clone();
    let size_event_change = size_event.clone();
    let size_callback_change = size_callback.clone();
    let attrs = shared_root_attrs(props);

    rsx! {
        nav { class: "{nav_class}", aria_label: "Pagination", ..attrs,
            div { class: "{row_class}", style: "height:auto",
                div { class: "{left_class}", style: "height:auto",
                    if let Some(spec) = total_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
                    if can_choose_size {
                        div { class: "{group_class}", style: "height:auto",
                            {registry.render_spec(&show_spec, None, &view)}
                            dioxus_primitives::select::Select::<String> {
                                default_value: page_size.map(|size| size.to_string()),
                                on_value_change: move |value: Option<String>| {
                                    if let Some(size) = value.and_then(|value| value.parse::<u32>().ok()) {
                                        emit_page_size(sink, &size_event_change, &size_callback_change, size);
                                    }
                                },
                                dioxus_primitives::select::SelectTrigger {
                                    aria_label: "Page size",
                                    class: "rounded-interactive border border-border bg-card px-2 py-1 text-sm",
                                    dioxus_primitives::select::SelectValue { placeholder: "Size" }
                                }
                                dioxus_primitives::select::SelectList {
                                    class: "absolute z-50 bg-card border border-border rounded-interactive shadow-elevation-popover",
                                    for (index, size) in size_options.iter().enumerate() {
                                        dioxus_primitives::select::SelectOption::<String> {
                                            key: "{size}", index: index, value: size.to_string(), "{size}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "{nav_group_class}", style: "height:auto",
                    {registry.render_spec(&previous, None, &view)}
                    {registry.render_spec(&page_of, None, &view)}
                    div { class: "{pages_class}", style: "height:auto",
                        for spec in page_specs.iter() {
                            {registry.render_spec(spec, None, &view)}
                        }
                    }
                    {registry.render_spec(&next, None, &view)}
                }
                if show_jump {
                    div { class: "{group_class}", style: "height:auto",
                        {registry.render_spec(&go_to, None, &view)}
                        input {
                            class: "block w-full rounded-interactive transition-all duration-fast border-[length:var(--border-width-thin)] border-border interactive-border h-input-md px-3 text-sm bg-card hover:bg-muted focus:bg-card text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring focus:border-ring disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-muted w-20",
                            r#type: "number", value: "{jump}", placeholder: "Page",
                            oninput: move |event| jump.set(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Enter {
                                    let entered = jump.read().clone();
                                    if let Ok(page) = entered.parse::<u32>() {
                                        if page >= 1 && (page as i64) <= total {
                                            emit_page(sink, &key_event, &key_callback, page);
                                            jump.set(String::new());
                                        }
                                    }
                                }
                            },
                        }
                        div { onclick: move |_| {
                                let entered = jump.read().clone();
                                if let Ok(page) = entered.parse::<u32>() {
                                    if page >= 1 && (page as i64) <= total {
                                        emit_page(sink, &click_event, &click_callback, page);
                                        jump.set(String::new());
                                    }
                                }
                            },
                            {registry.render_spec(&go_button, None, &view)}
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
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { PaginationFrame { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
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
