use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_string, get_string_or, };
use orbital_core::{Value, ValueMap};

pub struct FilterGroupRenderer;

fn filter_payload(entity: &str, field: &str, value: &str, query: Option<&str>) -> Value {
    let mut payload = ValueMap::new();
    payload.insert("entity".into(), Value::string(entity.to_string()));
    payload.insert("field".into(), Value::string(field.to_string()));
    payload.insert(
        "value".into(),
        Value::string(if value == "all" { "" } else { value }.to_string()),
    );
    if let Some(query) = query {
        payload.insert("query".into(), Value::string(query.to_string()));
    }
    Value::Object(payload.into())
}

fn clear_payload(entity: &str, query: Option<&str>) -> Value {
    let mut payload = ValueMap::new();
    payload.insert("entity".into(), Value::string(entity.to_string()));
    if let Some(query) = query {
        payload.insert("query".into(), Value::string(query.to_string()));
    }
    Value::Object(payload.into())
}

fn emit_filter(
    sink: Option<EventSink>,
    configured: &Option<String>,
    callback: &Option<String>,
    payload: Value,
) {
    if let Some(sink) = sink {
        if let Some(name) = configured {
            if name != "FILTER" {
                sink.call((name.clone(), payload.clone()));
            }
        }
        sink.call(("FILTER".into(), payload.clone()));
        if let Some(name) = callback {
            if name != "FILTER" && Some(name) != configured.as_ref() {
                sink.call((name.clone(), payload));
            }
        }
    }
}

fn emit_clear(
    sink: Option<EventSink>,
    configured: &Option<String>,
    callback: &Option<String>,
    payload: Value,
) {
    if let Some(sink) = sink {
        if let Some(name) = configured {
            if name != "CLEAR_FILTERS" {
                sink.call((name.clone(), payload.clone()));
            }
        }
        sink.call(("CLEAR_FILTERS".into(), payload));
        if let Some(name) = callback {
            if name != "CLEAR_FILTERS" && Some(name) != configured.as_ref() {
                sink.call((name.clone(), Value::Null));
            }
        }
    }
}

fn control_spec(
    kind: &str,
    label: &str,
    options: &[Value],
    filter: &Value,
    disabled: bool,
) -> Value {
    let mut fields = ValueMap::new();
    let pattern = match kind {
        "date" | "text" => "input",
        "numberrange" | "number-range" => "range-slider",
        _ => "select",
    };
    fields.insert("type".into(), Value::string(pattern));
    fields.insert("label".into(), Value::string(label.to_string()));
    fields.insert("disabled".into(), Value::Bool(disabled));
    if pattern == "input" {
        fields.insert(
            "inputType".into(),
            Value::string(if kind == "date" { "date" } else { "text" }),
        );
        fields.insert("placeholder".into(), Value::string(label.to_string()));
        fields.insert("onChange".into(), Value::string("FILTER_CONTROL"));
    } else if pattern == "range-slider" {
        for key in ["min", "max", "step"] {
            if let Some(value) = filter.as_object().and_then(|fields| fields.get(key)) {
                fields.insert(key.into(), value.clone());
            }
        }
        fields.insert("action".into(), Value::string("FILTER_CONTROL"));
    } else {
        let mut rows = Vec::with_capacity(options.len() + 1);
        let mut all = ValueMap::new();
        all.insert("value".into(), Value::string("all"));
        all.insert(
            "label".into(),
            Value::string(crate::i18n::t("filterGroup.all")),
        );
        rows.push(Value::Object(all.into()));
        for option in options {
            if let Some(text) = option.as_str() {
                let mut row = ValueMap::new();
                row.insert("value".into(), Value::string(text.to_string()));
                row.insert("label".into(), Value::string(text.to_string()));
                rows.push(Value::Object(row.into()));
            }
        }
        fields.insert("options".into(), Value::Array(rows.into()));
        fields.insert("value".into(), Value::string("all"));
        fields.insert("onValueChange".into(), Value::string("FILTER_CONTROL"));
    }
    Value::Object(fields.into())
}

fn control_value(payload: &Value) -> String {
    let value = payload.as_object().and_then(|fields| fields.get("value"));
    match value {
        Some(Value::String(text)) => text.as_str().to_string(),
        Some(Value::Int(number)) => number.to_string(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

fn render_control(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    spec: &Value,
    entity: &str,
    field: String,
    query: Option<String>,
    configured: Option<String>,
    callback: Option<String>,
) -> Element {
    let sink = view.sink;
    let entity = entity.to_string();
    let adapter: EventSink = Callback::new(move |(_name, value): (String, Value)| {
        let payload = filter_payload(&entity, &field, &control_value(&value), query.as_deref());
        emit_filter(sink, &configured, &callback, payload);
    });
    let adapted = view.for_source(view.source_trait.clone(), adapter);
    registry.render_spec(spec, None, &adapted)
}

fn render_pill(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    label: &str,
    value: &str,
    entity: &str,
    field: &str,
    query: Option<String>,
    configured: Option<String>,
    callback: Option<String>,
) -> Element {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::string("filter-pill"));
    fields.insert("label".into(), Value::string(label.to_string()));
    fields.insert("clickEvent".into(), Value::string("FILTER_CONTROL"));
    fields.insert("removable".into(), Value::Bool(false));
    let spec = Value::Object(fields.into());
    let entity = entity.to_string();
    let field = field.to_string();
    let value = value.to_string();
    let sink = view.sink;
    let adapter: EventSink = Callback::new(move |(_name, _payload): (String, Value)| {
        let payload = filter_payload(&entity, &field, &value, query.as_deref());
        emit_filter(sink, &configured, &callback, payload);
    });
    let adapted = view.for_source(view.source_trait.clone(), adapter);
    registry.render_spec(&spec, None, &adapted)
}

#[component]
fn FilterPopover(controls: Element, class: String) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "relative inline-block {class}",
            button { class: "rounded-interactive px-3 h-button-sm text-sm hover:bg-muted", aria_expanded: open(),
                onclick: move |_| open.set(!open()), {crate::i18n::t("filterGroup.filters")} }
            if open() {
                div { class: "absolute left-0 z-50 mt-2 min-w-[16rem] rounded-container border border-border bg-card p-card-md shadow-elevation-popover",
                    {controls}
                }
            }
        }
    }
}

impl DioxusPatternRenderer for FilterGroupRenderer {
    fn pattern_name(&self) -> &str {
        "filter-group"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let entity = get_string_or(props, "entity", "");
        let query = get_string(props, "query");
        let configured = get_string(props, "event");
        let callback = get_string(props, "onFilterChange");
        let clear_event = get_string(props, "clearEvent");
        let clear_callback = get_string(props, "onClearAll");
        let loading = get_bool_or(props, "isLoading", false);
        let variant = get_string_or(props, "variant", "default");
        let look = get_string_or(props, "look", "toolbar");
        let show_icon = get_bool_or(props, "showIcon", true);
        let filters = get_array(props, "filters");
        let vertical = variant == "vertical";
        let compact = variant == "compact";
        let pills = variant == "pills";
        let popover = look == "popover-trigger"
            || (look == "toolbar"
                && variant == "default"
                && view.viewport_width.is_some_and(|width| width < 768.0));
        let look_class = match look.as_str() {
            "chips" => "[&_input]:rounded-pill [&_select]:rounded-pill [&_button]:rounded-pill [&_input]:bg-muted [&_select]:bg-muted",
            "pills" => "[&_input]:rounded-pill [&_select]:rounded-pill [&_button]:rounded-pill",
            "inline-column-header" => "hidden",
            _ => "",
        };
        let layout = if vertical {
            "flex flex-col gap-4"
        } else if compact {
            "flex flex-wrap items-center gap-2"
        } else {
            "flex flex-wrap items-center gap-4"
        };
        let class = merge_class_name(&format!("{layout} {look_class}"), props);
        let sink = view.sink;
        let clear_payload = clear_payload(&entity, query.as_deref());
        let clear_action = clear_event.is_some() || clear_callback.is_some();
        let icon = Value::Object({
            let mut fields = ValueMap::new();
            fields.insert("type".into(), Value::string("icon"));
            fields.insert("name".into(), Value::string("filter"));
            fields.into()
        });
        let controls = rsx! {
            div { class: "{class}",
                if show_icon {
                    span { class: "inline-flex items-center gap-1 text-muted-foreground",
                        {registry.render_spec(&icon, None, view)}
                        if !compact && !pills { span { class: "text-xs font-bold uppercase tracking-wide", {crate::i18n::t("filterGroup.filters")} } }
                    }
                }
                for filter in filters.iter() {
                    { let field = get_string_or(filter, "field", "");
                      let label = get_string_or(filter, "label", "");
                      let kind = get_string(filter, "filterType").or_else(|| get_string(filter, "type")).unwrap_or_default();
                      let options = get_array(filter, "options");
                      let field_from = format!("{field}_from");
                      let field_to = format!("{field}_to");
                      let field_min = format!("{field}_min");
                      let field_max = format!("{field}_max");
                      let wrapper = if vertical { "flex flex-col gap-1" } else { "flex items-center gap-2" };
                      let default_spec = control_spec(&kind, &label, &options, filter, loading);
                      let from_spec = control_spec("date", &crate::i18n::t("filterGroup.from"), &[], filter, loading);
                      let to_spec = control_spec("date", &crate::i18n::t("filterGroup.to"), &[], filter, loading);
                      let min_spec = control_spec("number-range", &crate::i18n::t("filterGroup.from"), &[], filter, loading);
                      let max_spec = control_spec("number-range", &crate::i18n::t("filterGroup.to"), &[], filter, loading);
                      rsx! { div { key: "{field}", class: "{wrapper}",
                          if !compact { label { class: "text-xs font-bold text-muted-foreground uppercase tracking-wide", "{label}" } }
                          if pills {
                              div { class: "flex flex-wrap items-center gap-1",
                                  {render_pill(registry, view, &crate::i18n::t("filterGroup.all"), "all", &entity, &field, query.clone(), configured.clone(), callback.clone())}
                                  for option in options.iter() {
                                      if let Some(option) = option.as_str() {
                                          {render_pill(registry, view, option, option, &entity, &field, query.clone(), configured.clone(), callback.clone())}
                                      }
                                  }
                              }
                          } else if kind == "daterange" || kind == "date-range" {
                              div { class: "flex items-center gap-1",
                                  {render_control(registry, view, &from_spec, &entity, field_from, query.clone(), configured.clone(), callback.clone())}
                                  span { "–" }
                                  {render_control(registry, view, &to_spec, &entity, field_to, query.clone(), configured.clone(), callback.clone())}
                              }
                          } else if kind == "numberrange" || kind == "number-range" {
                              div { class: "flex items-center gap-1",
                                  {render_control(registry, view, &min_spec, &entity, field_min, query.clone(), configured.clone(), callback.clone())}
                                  span { "–" }
                                  {render_control(registry, view, &max_spec, &entity, field_max, query.clone(), configured.clone(), callback.clone())}
                              }
                          } else {
                              {render_control(registry, view, &default_spec, &entity, field, query.clone(), configured.clone(), callback.clone())}
                          }
                      } }
                    }
                }
                if clear_action {
                    button { class: "rounded-interactive px-3 h-button-sm text-sm hover:bg-muted", disabled: loading,
                        onclick: move |_| emit_clear(sink, &clear_event, &clear_callback, clear_payload.clone()),
                        {crate::i18n::t("filterGroup.clearAll")}
                    }
                }
            }
        };
        let attrs = shared_root_attrs(props);
        if popover {
            rsx! { div { ..attrs, FilterPopover { controls, class: String::new() } } }
        } else {
            rsx! { div { ..attrs, {controls} } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_event_uses_canonical_empty_value_and_declared_query() {
        let payload = filter_payload("Task", "status", "all", Some("@TaskQuery"));
        let fields = payload.as_object().unwrap();
        assert_eq!(fields.get("entity").and_then(Value::as_str), Some("Task"));
        assert_eq!(fields.get("field").and_then(Value::as_str), Some("status"));
        assert_eq!(fields.get("value").and_then(Value::as_str), Some(""));
        assert_eq!(
            fields.get("query").and_then(Value::as_str),
            Some("@TaskQuery")
        );
    }
}
