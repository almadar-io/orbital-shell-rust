//! Native Dioxus renderer for the `import-preview-tree` pattern: the web `ImportPreviewTree`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, merge_class_name, pattern_spec as pattern};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `import-preview-tree` pattern.
pub struct ImportPreviewTreeRenderer;

impl ImportPreviewTreeRenderer {
    const NAME: &str = "import-preview-tree";
}

impl DioxusPatternRenderer for ImportPreviewTreeRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let box_class = |extra: &str| cn(&["bg-transparent rounded-none shadow-none", extra]);
        let typo = |variant: &str, text: &str, class: &str| -> Value {
            let mut fields = vec![
                ("variant", Value::string(variant)),
                ("content", Value::string(text.to_string())),
            ];
            if !class.is_empty() {
                fields.push(("className", Value::string(class)));
            }
            pattern("typography", fields)
        };
        let root_class = merge_class_name(&box_class("flex flex-col gap-4"), props);
        fn fmt_field(v: &Value) -> String {
            match v {
                Value::Null => "—".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => {
                    if n.fract() == 0.0 {
                        format!("{}", *n as i64)
                    } else {
                        format!("{n}")
                    }
                }
                Value::Int(_) | Value::Bytes(_) => v.to_string_value(),
                Value::String(s) => s.clone().into_inner(),
                Value::Array(items) => items.iter().map(fmt_field).collect::<Vec<_>>().join(", "),
                Value::Object(_) => to_json(v).to_string(),
                Value::Lambda(_) => String::new(),
                Value::Sealed(_) => to_json(v).to_string(),
            }
        }
        let units = get_array(props, "units");
        let skipped = get_array(props, "skipped");
        let indent = get_number_or(props, "indent", 16.0);
        let display = props
            .as_object()
            .and_then(|o| o.get("entityDisplay"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
        for unit in units.iter() {
            let entity = get_string_or(unit, "targetEntity", "");
            match groups.iter_mut().find(|(name, _)| *name == entity) {
                Some(group) => group.1.push(unit.clone()),
                None => groups.push((entity, vec![unit.clone()])),
            }
        }
        let tree: Vec<_> = groups
            .iter()
            .map(|(entity, group)| {
                let label = display
                    .as_object()
                    .and_then(|d| d.get(entity))
                    .and_then(|d| get_string(d, "plural"))
                    .unwrap_or_else(|| entity.clone());
                let refs: Vec<String> = group.iter().map(|u| get_string_or(u, "ref", "")).collect();
                let is_child = |u: &Value| {
                    get_string(u, "parentRef").is_some_and(|p| !p.is_empty() && refs.contains(&p))
                };
                let mut stack: Vec<(&Value, usize)> = group
                    .iter()
                    .rev()
                    .filter(|u| !is_child(u))
                    .map(|u| (u, 0))
                    .collect();
                let mut rows: Vec<(f64, String, String, String)> = Vec::new();
                while let Some((unit, depth)) = stack.pop() {
                    let uref = get_string_or(unit, "ref", "");
                    let title = get_string(unit, "title")
                        .filter(|t| !t.is_empty())
                        .unwrap_or_else(|| uref.clone());
                    let mut entries: Vec<(String, String)> = unit
                        .as_object()
                        .and_then(|o| o.get("fields"))
                        .and_then(|f| f.as_object())
                        .map(|f| {
                            f.iter()
                                .map(|(k, v)| (k.to_string(), fmt_field(v)))
                                .collect()
                        })
                        .unwrap_or_default();
                    entries.sort_by(|a, b| a.0.cmp(&b.0));
                    let summary = entries
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    rows.push((depth as f64 * indent, title, summary, uref.clone()));
                    for child in group.iter().rev() {
                        if get_string(child, "parentRef").as_deref() == Some(uref.as_str()) {
                            stack.push((child, depth + 1));
                        }
                    }
                }
                (label, group.len(), rows)
            })
            .collect();
        let confirm_event = get_string(props, "onConfirm").filter(|e| !e.is_empty());
        let cancel_event = get_string(props, "onCancel").filter(|e| !e.is_empty());
        let action_button = |label: String, variant: &str, action: String| -> Value {
            let mut m = orbital_core::ValueMap::new();
            m.insert(
                "type".to_string().into(),
                Value::String(("button".to_string()).into()),
            );
            m.insert("label".to_string().into(), Value::String(label.into()));
            m.insert(
                "variant".to_string().into(),
                Value::String(variant.to_string().into()),
            );
            m.insert("action".to_string().into(), Value::String(action.into()));
            Value::Object(m.into())
        };
        let count_badge = |n: usize| -> Value {
            let mut m = orbital_core::ValueMap::new();
            m.insert(
                "type".to_string().into(),
                Value::String(("badge".to_string()).into()),
            );
            m.insert("amount".to_string().into(), Value::Number(n as f64));
            Value::Object(m.into())
        };
        let cancel_button = cancel_event
            .map(|e| action_button(get_string_or(props, "cancelLabel", "Cancel"), "default", e));
        let confirm_button = confirm_event.map(|e| {
            action_button(
                get_string_or(props, "confirmLabel", "Confirm import"),
                "primary",
                e,
            )
        });
        let skipped_badge = count_badge(skipped.len());
        let skipped_rows: Vec<(String, String)> = skipped
            .iter()
            .map(|e| (get_string_or(e, "ref", ""), get_string_or(e, "reason", "")))
            .collect();
        let tile = box_class("rounded-container border border-border p-3");
        let row_class = box_class("flex flex-col py-1");
        let line_class = box_class("flex items-center gap-2");
        let head_class = box_class("flex items-center gap-2 pb-2");
        let skip_class = box_class("flex items-baseline gap-2 py-1");
        let actions_class = box_class("flex items-center justify-end gap-2");
        let file_icon = pattern(
            "icon",
            [
                ("name", Value::string("file-text")),
                ("size", Value::string("xs")),
                ("className", Value::string("text-muted-foreground")),
            ],
        );
        let empty_text = typo("body2", "No units staged.", "text-muted-foreground");
        let skipped_label = typo("label", "Skipped", "");
        rsx! {
            div { class: "{root_class}",
                if units.is_empty() { {registry.render_spec(&empty_text, None, view)} }
                for (label, count, rows) in tree.iter() {
                    div { class: "{tile}",
                        div { class: "{head_class}",
                            {registry.render_spec(&typo("label", label, ""), None, view)}
                            {registry.render_spec(&count_badge(*count), None, view)}
                        }
                        for (pad, title, summary, uref) in rows.iter() {
                            div { key: "{uref}", class: "{row_class}", style: "padding-left:{pad}px", "data-testid": "import-preview-unit-{uref}",
                                div { class: "{line_class}",
                                    {registry.render_spec(&file_icon, None, view)}
                                    {registry.render_spec(&typo("body2", title, ""), None, view)}
                                }
                                if !summary.is_empty() { {registry.render_spec(&typo("caption", summary, "text-muted-foreground"), None, view)} }
                            }
                        }
                    }
                }
                if !skipped.is_empty() {
                    div { class: "{tile}",
                        div { class: "{head_class}",
                            {registry.render_spec(&skipped_label, None, view)}
                            {registry.render_spec(&skipped_badge, None, view)}
                        }
                        for (skipped_ref, reason) in skipped_rows.iter() {
                            div { key: "{skipped_ref}", class: "{skip_class}",
                                {registry.render_spec(&typo("body2", skipped_ref, ""), None, view)}
                                {registry.render_spec(&typo("caption", reason, "text-muted-foreground"), None, view)}
                            }
                        }
                    }
                }
                if cancel_button.is_some() || confirm_button.is_some() {
                    div { class: "{actions_class}",
                        if let Some(b) = cancel_button.as_ref() { {registry.render_spec(b, None, view)} }
                        if let Some(b) = confirm_button.as_ref() { {registry.render_spec(b, None, view)} }
                    }
                }
            }
        }
    }
}
