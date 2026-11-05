//! Dioxus renderer for the shared `breadcrumb` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct BreadcrumbRenderer;

fn icon_spec(name: &str) -> Value {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::String("icon".into()));
    fields.insert("name".into(), Value::String(name.to_string().into()));
    fields.insert("size".into(), Value::String("sm".into()));
    Value::Object(fields.into())
}

fn button_spec(item: &Value, index: usize, label: &str, href: Option<&str>) -> Value {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::String("button".into()));
    fields.insert("variant".into(), Value::String("link".into()));
    fields.insert("label".into(), Value::String(label.to_string().into()));
    fields.insert(
        "className".into(),
        Value::String("h-auto gap-1.5 min-w-0 no-underline text-muted-foreground".into()),
    );
    if let Some(icon) = get_string(item, "icon") {
        fields.insert("icon".into(), Value::String(icon.into()));
    }
    if let Some(event) = get_string(item, "event") {
        let mut payload = ValueMap::new();
        payload.insert("label".into(), Value::String(label.to_string().into()));
        payload.insert("index".into(), Value::Int(index as i64));
        if let Some(href) = href {
            payload.insert("href".into(), Value::String(href.to_string().into()));
        }
        fields.insert("action".into(), Value::String(event.into()));
        fields.insert("actionPayload".into(), Value::Object(payload.into()));
    } else if let Some(href) = href {
        fields.insert("href".into(), Value::String(href.to_string().into()));
    }
    Value::Object(fields.into())
}

impl DioxusPatternRenderer for BreadcrumbRenderer {
    fn pattern_name(&self) -> &str {
        "breadcrumb"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        if get_bool_or(props, "fromNavStack", false) {
            return rsx! {};
        }
        let items = get_array(props, "items");
        let max_items = get_number(props, "maxItems")
            .filter(|value| *value > 0.0)
            .map(|value| value as usize);
        let crumbs: Vec<Option<(usize, &Value)>> = if let Some(max) = max_items {
            if items.len() > max {
                let mut visible = vec![Some((0, &items[0])), None];
                visible.extend(
                    items
                        .iter()
                        .enumerate()
                        .skip(items.len() - (max - 1))
                        .map(Some),
                );
                visible
            } else {
                items.iter().enumerate().map(Some).collect()
            }
        } else {
            items.iter().enumerate().map(Some).collect()
        };
        let has_explicit_current = items
            .iter()
            .any(|item| get_bool_or(item, "isCurrent", false));
        let separator = icon_spec(&get_string_or(props, "separator", "chevron-right"));
        let class = merge_class_name("min-w-0", props);
        rsx! {
            nav { class: "{class}", ..shared_root_attrs(props),
                ol { class: "flex flex-wrap items-center gap-x-2 gap-y-1 min-w-0",
                    for (position, crumb) in crumbs.iter().enumerate() {
                        {match crumb {
                            Some((index, item)) => {
                                let label = get_string_or(item, "label", "");
                                let href = get_string(item, "path").or_else(|| get_string(item, "href"));
                                let current = if has_explicit_current {
                                    get_bool_or(item, "isCurrent", false)
                                } else {
                                    position == crumbs.len() - 1
                                };
                                let icon = get_string(item, "icon").map(|name| icon_spec(&name));
                                let button = button_spec(item, *index, &label, href.as_deref());
                                rsx! { li { class: "flex items-center gap-2 min-w-0",
                                    if current {
                                        span { class: "flex items-center gap-1.5 min-w-0 text-foreground", aria_current: "page",
                                            if let Some(icon) = icon.as_ref() { {registry.render_spec(icon, None, view)} }
                                            span { class: "text-sm font-medium truncate max-w-[16rem]", "{label}" }
                                        }
                                    } else {
                                        {registry.render_spec(&button, None, view)}
                                    }
                                    if position + 1 < crumbs.len() {
                                        span { class: "inline-flex text-muted-foreground rtl:-scale-x-100", aria_hidden: "true", {registry.render_spec(&separator, None, view)} }
                                    }
                                } }
                            }
                            None => rsx! { li { class: "flex items-center gap-2", span { class: "text-sm text-muted-foreground", aria_hidden: "true", "…" }
                                span { class: "inline-flex text-muted-foreground rtl:-scale-x-100", aria_hidden: "true", {registry.render_spec(&separator, None, view)} }
                            } },
                        }}
                    }
                }
            }
        }
    }
}
