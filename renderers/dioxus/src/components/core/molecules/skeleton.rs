use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number_or, get_string, get_string_or};
use orbital_core::Value;

pub struct SkeletonRenderer;

const PULSE: &str = "almadar-shimmer animate-shimmer rounded-container";

fn gap_class(gap: &str) -> &'static str {
    match gap {
        "none" => "gap-0",
        "xs" => "gap-1",
        "sm" => "gap-2",
        "lg" => "gap-6",
        _ => "gap-4",
    }
}

fn stack(direction: &str, gap: &str, class: &str, children: Element) -> Element {
    let class = cn(&[
        "flex",
        direction,
        gap_class(gap),
        "items-stretch justify-start",
        class,
    ]);
    rsx! { div { class: "{class}", {children} } }
}

fn vstack(gap: &str, class: &str, children: Element) -> Element {
    stack("flex-col", gap, class, children)
}

fn hstack(gap: &str, class: &str, children: Element) -> Element {
    stack("flex-row", gap, class, children)
}

fn block(class: &str) -> Element {
    let class = cn(&[BOX_ATOM_BASE, &cn(&[PULSE, class])]);
    rsx! { div { class: "{class}" } }
}

fn line(class: &str) -> Element {
    block(&cn(&["h-4", class]))
}

fn grid(min_width: &str, max_cols: Option<usize>, class: &str, children: Element) -> Element {
    let class = cn(&[BOX_ATOM_BASE, "grid", "gap-4", class]);
    let template = match max_cols {
        Some(max) => format!(
            "repeat(auto-fit, minmax(max(min({min_width}, 100%), calc((100% - {} * 1rem) / {max})), 1fr))",
            max - 1
        ),
        None => format!("repeat(auto-fit, minmax({min_width}, 1fr))"),
    };
    let style = format!("grid-template-columns:{template}");
    rsx! { div { class: "{class}", style: "{style}", {children} } }
}

fn header(class: &str) -> Element {
    let row = cn(&["items-center justify-between px-6 py-5", class]);
    hstack(
        "md",
        &row,
        rsx! {
            {vstack("sm", "flex-1", rsx! { {block("h-7 w-48")} {line("w-64")} })}
            {hstack("sm", "", rsx! { {block("h-9 w-24 rounded-interactive")} {block("h-9 w-32 rounded-interactive")} })}
        },
    )
}

fn table(rows: usize, columns: usize, class: &str) -> Element {
    let outer = cn(&[
        "border border-border rounded-container overflow-hidden",
        class,
    ]);
    vstack(
        "none",
        &outer,
        rsx! {
            {hstack("md", "px-4 py-3 bg-muted/30 border-b border-border", rsx! {
                for _ in 0..columns { {block("h-4 flex-1 mx-2")} }
            })}
            for row in 0..rows {
                {hstack("md", if row + 1 < rows { "px-4 py-3 border-b border-border" } else { "px-4 py-3" }, rsx! {
                    for _ in 0..columns { {line("flex-1 mx-2")} }
                })}
            }
        },
    )
}

fn form(fields: usize, class: &str) -> Element {
    let outer = cn(&["p-6", class]);
    vstack(
        "lg",
        &outer,
        rsx! {
            {block("h-6 w-40")}
            for _ in 0..fields {
                {vstack("sm", "", rsx! { {block("h-4 w-24")} {block("h-10 w-full rounded-interactive")} })}
            }
            {hstack("md", "justify-end pt-2", rsx! {
                {block("h-10 w-20 rounded-interactive")} {block("h-10 w-24 rounded-interactive")}
            })}
        },
    )
}

fn card(class: &str) -> Element {
    let outer = cn(&["p-5 border border-border rounded-container", class]);
    vstack(
        "md",
        &outer,
        rsx! {
            {hstack("md", "items-center", rsx! {
                {block("h-10 w-10 rounded-full")}
                {vstack("xs", "flex-1", rsx! { {block("h-5 w-32")} {line("w-48")} })}
            })}
            {line("w-full")}
            {line("w-3/4")}
            {hstack("sm", "pt-2", rsx! { {block("h-6 w-16 rounded-full")} {block("h-6 w-20 rounded-full")} })}
        },
    )
}

fn text(rows: usize, class: &str) -> Element {
    vstack(
        "sm",
        class,
        rsx! {
            for index in 0..rows { {line(if index + 1 == rows { "w-2/3" } else { "w-full" })} }
        },
    )
}

fn list(rows: usize, class: &str) -> Element {
    let outer = cn(&[
        "border border-border rounded-container overflow-hidden",
        class,
    ]);
    vstack(
        "none",
        &outer,
        rsx! {
            for row in 0..rows {
                {hstack("md", if row + 1 < rows { "items-center px-4 py-3 border-b border-border" } else { "items-center px-4 py-3" }, rsx! {
                    {block("h-10 w-10 rounded-full shrink-0")}
                    {vstack("xs", "flex-1 min-w-0", rsx! { {block("h-4 w-1/3")} {line("w-2/3 h-3")} })}
                    {block("h-6 w-16 rounded-full")}
                })}
            }
        },
    )
}

fn grid_cards(rows: usize, class: &str) -> Element {
    grid(
        "256px",
        None,
        class,
        rsx! { for _ in 0..rows { {card("")} } },
    )
}

fn stats(columns: usize, class: &str) -> Element {
    grid(
        "176px",
        None,
        class,
        rsx! {
            for _ in 0..columns {
                {vstack("sm", "p-4 border border-border rounded-container", rsx! { {line("w-1/2 h-3")} {block("h-7 w-2/3")} })}
            }
        },
    )
}

fn detail(rows: usize, class: &str) -> Element {
    let outer = cn(&["p-6 border border-border rounded-container", class]);
    vstack(
        "lg",
        &outer,
        rsx! {
            {hstack("md", "items-start justify-between", rsx! {
                {hstack("sm", "items-center", rsx! { {block("h-8 w-56")} {block("h-6 w-20 rounded-full")} })}
                {hstack("sm", "", rsx! { {block("h-9 w-24 rounded-interactive")} {block("h-9 w-9 rounded-interactive")} })}
            })}
            {hstack("md", "", rsx! { {block("h-20 flex-1")} {block("h-20 flex-1")} {block("h-20 flex-1")} })}
            {grid("192px", Some(3), "", rsx! {
                for _ in 0..rows {
                    {vstack("xs", "", rsx! { {line("w-1/3 h-3")} {block("h-5 w-2/3")} })}
                }
            })}
            {text(3, "")}
        },
    )
}

fn dimension(value: &Value, key: &str, default: usize) -> usize {
    get_number_or(value, key, default as f64).max(0.0) as usize
}

fn variant_of(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            value
                .get("variant")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "text".to_string())
}

fn render_variant(variant: &str, shape: &Value, class: &str) -> Element {
    let rows = |default: usize| dimension(shape, "rows", default);
    let columns = dimension(shape, "columns", 4);
    match variant {
        "header" => header(class),
        "table" => table(rows(5), columns, class),
        "list" => list(rows(5), class),
        "grid" => grid_cards(rows(6), class),
        "detail" => detail(rows(6), class),
        "stats" => stats(columns, class),
        "form" => form(dimension(shape, "fields", 4), class),
        "card" => card(class),
        _ => text(rows(3), class),
    }
}

fn render_node(node: &Value) -> Element {
    if let Some(children) = node.get("stack").and_then(Value::as_array) {
        return vstack(
            "lg",
            "",
            rsx! { for child in children.iter() { {render_node(child)} } },
        );
    }
    let shape = node.get("shape").unwrap_or(node);
    render_variant(&variant_of(shape), shape, "")
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
        let label = get_string(props, "aria-label").unwrap_or_else(|| "Loading...".to_string());
        let class_name = get_string_or(props, "className", "");
        let is_tree = spec.is_some_and(|value| value.get("stack").is_some());
        let (variant, content) = match spec {
            Some(spec) if is_tree => ("tree".to_string(), render_node(spec)),
            Some(spec) => (
                variant_of(spec),
                render_variant(&variant_of(spec), spec, class_name.trim()),
            ),
            None => {
                let variant = get_string_or(props, "variant", "text");
                let content = render_variant(&variant, props, class_name.trim());
                (variant, content)
            }
        };
        let outer = if is_tree {
            cn(&[
                BOX_ATOM_BASE,
                "w-full almadar-awaiting-skeleton",
                class_name.trim(),
            ])
        } else {
            cn(&[BOX_ATOM_BASE, "w-full"])
        };
        rsx! {
            div {
                class: "{outer}",
                role: "status",
                "aria-busy": "true",
                "aria-label": "{label}",
                "data-skeleton": "{variant}",
                "data-loading-state": "",
                {content}
            }
        }
    }
}
