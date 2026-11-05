use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number_or, get_string, get_string_or, merge_class_name};
use orbital_core::Value;

pub struct SkeletonRenderer;

fn block(width: &str, height: &str) -> Element {
    rsx! { div { class: "almadar-shimmer animate-shimmer", style: "width:{width};height:{height};border-radius:6px;flex-shrink:0" } }
}

fn shape(variant: &str, rows: usize, columns: usize, fields: usize) -> Element {
    match variant {
        "header" => rsx! {
            div { style: "display:flex;justify-content:space-between;align-items:center;padding:20px 24px;gap:16px",
                div { style: "display:flex;flex-direction:column;gap:8px;flex:1", {block("192px", "28px")} {block("256px", "16px")} }
                div { style: "display:flex;gap:8px", {block("96px", "36px")} {block("128px", "36px")} }
            }
        },
        "table" => rsx! {
            div { style: "border:1px solid var(--almadar-line);border-radius:8px;overflow:hidden",
                div { style: "display:flex;gap:12px;padding:12px 16px;background:var(--almadar-accent-bg)",
                    for _ in 0..columns { div { style: "flex:1", {block("100%", "16px")} } }
                }
                for _ in 0..rows {
                    div { style: "display:flex;gap:12px;padding:12px 16px;border-top:1px solid var(--almadar-line)",
                        for _ in 0..columns { div { style: "flex:1", {block("100%", "16px")} } }
                    }
                }
            }
        },
        "list" => rsx! {
            div { style: "border:1px solid var(--almadar-line);border-radius:8px;overflow:hidden",
                for _ in 0..rows {
                    div { style: "display:flex;align-items:center;gap:16px;padding:12px 16px;border-bottom:1px solid var(--almadar-line)",
                        {block("40px", "40px")}
                        div { style: "display:flex;flex:1;flex-direction:column;gap:6px", {block("33%", "16px")} {block("66%", "12px")} }
                        {block("64px", "24px")}
                    }
                }
            }
        },
        "grid" => rsx! {
            div { style: "display:grid;grid-template-columns:repeat(auto-fit,minmax(256px,1fr));gap:16px",
                for _ in 0..rows {
                    div { style: "display:flex;flex-direction:column;gap:12px;padding:20px;border:1px solid var(--almadar-line);border-radius:8px",
                        {block("40px", "40px")} {block("70%", "20px")} {block("100%", "16px")} {block("80%", "16px")}
                    }
                }
            }
        },
        "detail" => rsx! {
            div { style: "display:flex;flex-direction:column;gap:20px;padding:24px;border:1px solid var(--almadar-line);border-radius:8px",
                div { style: "display:flex;justify-content:space-between;gap:12px", {block("224px", "32px")} {block("96px", "36px")} }
                div { style: "display:flex;gap:16px", for _ in 0..3 { div { style: "flex:1", {block("100%", "80px")} } } }
                div { style: "display:grid;grid-template-columns:repeat(auto-fit,minmax(192px,1fr));gap:16px",
                    for _ in 0..rows { div { style: "display:flex;flex-direction:column;gap:8px", {block("33%", "12px")} {block("66%", "20px")} } }
                }
            }
        },
        "stats" => rsx! {
            div { style: "display:grid;grid-template-columns:repeat(auto-fit,minmax(176px,1fr));gap:16px",
                for _ in 0..columns {
                    div { style: "display:flex;flex-direction:column;gap:8px;padding:16px;border:1px solid var(--almadar-line);border-radius:8px",
                        {block("50%", "12px")} {block("66%", "28px")}
                    }
                }
            }
        },
        "form" => rsx! {
            div { style: "display:flex;flex-direction:column;gap:20px;padding:24px",
                {block("160px", "24px")}
                for _ in 0..fields { div { style: "display:flex;flex-direction:column;gap:8px", {block("96px", "16px")} {block("100%", "40px")} } }
                div { style: "display:flex;justify-content:flex-end;gap:12px", {block("80px", "40px")} {block("96px", "40px")} }
            }
        },
        "card" => rsx! {
            div { style: "display:flex;flex-direction:column;gap:12px;padding:20px;border:1px solid var(--almadar-line);border-radius:8px",
                div { style: "display:flex;align-items:center;gap:12px", {block("40px", "40px")} {block("128px", "20px")} }
                {block("100%", "16px")} {block("75%", "16px")} {block("64px", "24px")}
            }
        },
        _ => rsx! {
            div { style: "display:flex;flex-direction:column;gap:8px",
                for index in 0..rows { {block(if index + 1 == rows { "66%" } else { "100%" }, "16px")} }
            }
        },
    }
}

fn dimension(value: &Value, key: &str, default: usize) -> usize {
    get_number_or(value, key, default as f64).max(0.0) as usize
}

fn render_node(value: &Value) -> Element {
    if value.as_str() == Some("none") {
        return rsx! {};
    }
    if let Some(stack) = value.get("stack").and_then(Value::as_array) {
        return rsx! { div { style: "display:flex;flex-direction:column;gap:24px", for child in stack { {render_node(child)} } } };
    }
    let value = value.get("shape").unwrap_or(value);
    let variant = value
        .as_str()
        .or_else(|| value.get("variant").and_then(Value::as_str))
        .unwrap_or("text");
    let rows = dimension(
        value,
        "rows",
        match variant {
            "table" | "list" => 5,
            "grid" | "detail" => 6,
            _ => 3,
        },
    );
    shape(
        variant,
        rows,
        dimension(value, "columns", 4),
        dimension(value, "fields", 4),
    )
}

impl DioxusPatternRenderer for SkeletonRenderer {
    fn pattern_name(&self) -> &str {
        "skeleton"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let spec = props.get("spec");
        if spec.is_some_and(|value| value.as_str() == Some("none")) {
            return rsx! {};
        }
        let class = merge_class_name("almadar-skeleton", props);
        let label = get_string(props, "aria-label");
        let variant = spec
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| get_string_or(props, "variant", "text"));
        let content = if let Some(spec) = spec {
            render_node(spec)
        } else {
            shape(
                &variant,
                dimension(props, "rows", 3),
                dimension(props, "columns", 4),
                dimension(props, "fields", 4),
            )
        };
        let data_variant = if spec.is_some_and(|value| value.get("stack").is_some()) {
            "tree"
        } else {
            spec.and_then(|value| value.get("shape").or(Some(value)))
                .and_then(|value| {
                    value
                        .get("variant")
                        .and_then(Value::as_str)
                        .or_else(|| value.as_str())
                })
                .unwrap_or(&variant)
        };
        rsx! { div { class: "{class}", role: "status", "aria-busy": "true", "aria-label": label, "data-skeleton": "{data_variant}", style: "width:100%", {content} } }
    }
}
