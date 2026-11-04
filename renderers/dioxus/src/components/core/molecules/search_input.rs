use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView};
use crate::timer::Delay;
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number_or, get_string, get_string_or, merge_class_name,
};
use orbital_core::{Value, ValueMap};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct SearchInputRenderer;

#[derive(Props, Clone, PartialEq)]
struct SearchInputProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn search_payload(term: &str, entity: Option<&str>, query: Option<&str>) -> Value {
    let mut payload = ValueMap::new();
    payload.insert("searchTerm".into(), Value::String(term.to_string().into()));
    if let Some(entity) = entity {
        payload.insert("entity".into(), Value::String(entity.to_string().into()));
    }
    if let Some(query) = query {
        payload.insert("query".into(), Value::String(query.to_string().into()));
    }
    Value::Object(payload.into())
}

fn search_events(
    event: Option<&str>,
    entity: Option<&str>,
    query: Option<&str>,
    term: &str,
) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    if let Some(event) = event.filter(|event| *event != "SEARCH") {
        events.push((event.to_string(), search_payload(term, entity, None)));
    }
    if event.is_some() || entity.is_some() || query.is_some() {
        events.push(("SEARCH".to_string(), search_payload(term, entity, query)));
    }
    events
}

fn clear_event(
    event: Option<&str>,
    entity: Option<&str>,
    query: Option<&str>,
) -> Option<(String, Value)> {
    if event.is_some() || query.is_some() {
        Some((
            "CLEAR_SEARCH".to_string(),
            search_payload("", entity, query),
        ))
    } else {
        None
    }
}

#[allow(non_snake_case)]
fn SearchInput(props: SearchInputProps) -> Element {
    let controlled = get_string(&props.props, "value");
    let initial = controlled.clone().unwrap_or_default();
    let mut text = use_signal(move || initial);
    let mut generation = use_signal(|| 0_u64);
    let initial_controlled = controlled.clone();
    let previous_controlled = use_hook(move || Rc::new(RefCell::new(initial_controlled)));
    if *previous_controlled.borrow() != controlled {
        *previous_controlled.borrow_mut() = controlled.clone();
        if let Some(value) = controlled {
            text.set(value);
        }
    }

    let class = merge_class_name("relative w-full", &props.props);
    let placeholder = get_string_or(&props.props, "placeholder", "Search...");
    let is_loading = get_bool_or(&props.props, "isLoading", false);
    let clearable = get_bool_or(&props.props, "clearable", true) && !is_loading;
    let debounce_ms = get_number_or(&props.props, "debounceMs", 300.0).max(0.0) as u64;
    let event = get_string(&props.props, "event").filter(|value| !value.is_empty());
    let entity = get_string(&props.props, "entity").filter(|value| !value.is_empty());
    let query = get_string(&props.props, "query").filter(|value| !value.is_empty());
    let sink = props.view.sink;

    let relay: EventSink =
        Callback::new(
            move |(name, payload): (String, Value)| match name.as_str() {
                "__search_input_change" => {
                    let Some(value) = get_string(&payload, "value") else {
                        return;
                    };
                    text.set(value.clone());
                    let next = generation.peek().wrapping_add(1);
                    generation.set(next);
                    let event = event.clone();
                    let entity = entity.clone();
                    let query = query.clone();
                    spawn(async move {
                        Delay::new(Duration::from_millis(debounce_ms)).await;
                        if *generation.peek() == next {
                            if let Some(sink) = sink {
                                for event in search_events(
                                    event.as_deref(),
                                    entity.as_deref(),
                                    query.as_deref(),
                                    &value,
                                ) {
                                    sink.call(event);
                                }
                            }
                        }
                    });
                }
                "__search_input_clear" => {
                    let next = generation.peek().wrapping_add(1);
                    generation.set(next);
                    text.set(String::new());
                    if let (Some(sink), Some(event)) = (
                        sink,
                        clear_event(event.as_deref(), entity.as_deref(), query.as_deref()),
                    ) {
                        sink.call(event);
                    }
                }
                _ => {}
            },
        );

    let mut input = ValueMap::new();
    input.insert("type".into(), Value::String("input".into()));
    input.insert("inputType".into(), Value::String("search".into()));
    input.insert("value".into(), Value::String(text().into()));
    input.insert("placeholder".into(), Value::String(placeholder.into()));
    input.insert("leftIcon".into(), Value::String("search".into()));
    input.insert("clearable".into(), Value::Bool(clearable));
    input.insert(
        "onChange".into(),
        Value::String("__search_input_change".into()),
    );
    input.insert(
        "onClear".into(),
        Value::String("__search_input_clear".into()),
    );
    input.insert("className".into(), Value::String("pr-10".into()));
    input.insert("aria-busy".into(), Value::Bool(is_loading));
    for key in [
        "aria-label",
        "aria-labelledby",
        "aria-describedby",
        "role",
        "lang",
        "dir",
        "tabIndex",
    ] {
        if let Some(value) = props.props.as_object().and_then(|fields| fields.get(key)) {
            input.insert(key.into(), value.clone());
        }
    }
    let input = Value::Object(input.into());
    let mut spinner = ValueMap::new();
    spinner.insert("type".into(), Value::String("spinner".into()));
    spinner.insert("size".into(), Value::String("sm".into()));
    spinner.insert("color".into(), Value::String("primary".into()));
    let spinner = Value::Object(spinner.into());
    let view = props.view.as_view();
    let input_view = view.for_source(view.source_trait.clone(), relay);
    rsx! {
        div { class: "{class}",
            {props.registry.render_spec(&input, None, &input_view)}
            if is_loading {
                div { class: "absolute right-3 top-1/2 -translate-y-1/2",
                    {props.registry.render_spec(&spinner, None, &view)}
                }
            }
        }
    }
}

impl DioxusPatternRenderer for SearchInputRenderer {
    fn pattern_name(&self) -> &str {
        "search-input"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { SearchInput { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_payload_keeps_declared_context() {
        let payload = search_payload("task", Some("Task"), Some("@TaskQuery"));
        let fields = payload.as_object().unwrap();
        assert_eq!(
            fields.get("searchTerm").and_then(Value::as_str),
            Some("task")
        );
        assert_eq!(fields.get("entity").and_then(Value::as_str), Some("Task"));
        assert_eq!(
            fields.get("query").and_then(Value::as_str),
            Some("@TaskQuery")
        );
        let no_context = search_payload("task", None, None);
        let fields = no_context.as_object().unwrap();
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn search_and_clear_events_keep_the_declared_payloads() {
        let events = search_events(
            Some("CATALOG_SEARCH"),
            Some("Task"),
            Some("@TaskQuery"),
            "x",
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "CATALOG_SEARCH");
        assert!(events[0].1.as_object().unwrap().get("query").is_none());
        assert_eq!(events[1].0, "SEARCH");
        assert_eq!(
            events[1]
                .1
                .as_object()
                .unwrap()
                .get("query")
                .and_then(Value::as_str),
            Some("@TaskQuery")
        );
        assert_eq!(search_events(Some("SEARCH"), None, None, "x").len(), 1);
        assert!(search_events(None, None, None, "x").is_empty());
        let clear = clear_event(Some("CATALOG_SEARCH"), Some("Task"), Some("@TaskQuery")).unwrap();
        assert_eq!(clear.0, "CLEAR_SEARCH");
        assert_eq!(
            clear
                .1
                .as_object()
                .unwrap()
                .get("searchTerm")
                .and_then(Value::as_str),
            Some("")
        );
        assert!(clear_event(None, Some("Task"), None).is_none());
    }
}
