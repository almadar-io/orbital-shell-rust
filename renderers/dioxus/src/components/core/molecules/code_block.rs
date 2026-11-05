//! Native body of the `code-block` pattern, mirroring the web `CodeBlock` molecule (read-only
//! standard block, viewer mode with files / diff / line numbers). Syntax colouring is not carried:
//! no Rust highlighter is a dependency, so code paints as the web does before its highlighter loads.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    compute_line_diff, object_value, pattern_spec, typography_spec, DioxusPatternRenderer,
    DioxusView, EventSink, LineDiffKind, OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `code-block` pattern.
pub struct CodeBlockRenderer;

impl CodeBlockRenderer {
    const NAME: &str = "code-block";
}

const CARD: &str = "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md";
const MONO_FONT: &str = "ui-monospace, SFMono-Regular, Menlo, Monaco, \"Cascadia Mono\", \"Courier New\", \"IBM Plex Sans Arabic\", \"Noto Sans Arabic\", monospace";
const CHUNK_LINES: usize = 400;

#[derive(Props, Clone, PartialEq)]
struct BlockProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

struct DiffRow {
    bg: &'static str,
    prefix: &'static str,
    text: &'static str,
    content: String,
    line_number: Option<usize>,
}

fn diff_style(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "add" | "added" => ("bg-success/10", "+", "text-success"),
        "remove" | "removed" => ("bg-error/10", "-", "text-error"),
        _ => ("", " ", "text-foreground"),
    }
}

fn diff_rows(props: &Value) -> Option<Vec<DiffRow>> {
    let authored = get_array(props, "diff");
    if !authored.is_empty() {
        return Some(
            authored
                .iter()
                .map(|line| {
                    let (bg, prefix, text) = diff_style(&get_string_or(line, "type", ""));
                    DiffRow {
                        bg,
                        prefix,
                        text,
                        content: get_string_or(line, "content", ""),
                        line_number: get_number(line, "lineNumber").map(|n| n as usize),
                    }
                })
                .collect(),
        );
    }
    if get_string_or(props, "mode", "code") == "diff" {
        let old = get_string(props, "oldValue")?;
        let new = get_string(props, "newValue")?;
        return Some(
            compute_line_diff(&old, &new)
                .into_iter()
                .map(|line| {
                    let kind = match line.kind {
                        LineDiffKind::Added => "added",
                        LineDiffKind::Removed => "removed",
                        LineDiffKind::Unchanged => "unchanged",
                    };
                    let (bg, prefix, text) = diff_style(kind);
                    DiffRow {
                        bg,
                        prefix,
                        text,
                        content: line.content,
                        line_number: line.after_line_number.or(line.before_line_number),
                    }
                })
                .collect(),
        );
    }
    None
}

fn max_height(props: &Value) -> String {
    match props.get("maxHeight") {
        Some(value) => match value.as_str() {
            Some(text) => text.to_string(),
            None => value
                .as_number()
                .map(|number| format!("{number}px"))
                .unwrap_or_else(|| "60vh".to_string()),
        },
        None => "60vh".to_string(),
    }
}

fn plain_chunks(code: &str, style: &str) -> Element {
    let lines: Vec<&str> = code.split('\n').collect();
    let widest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let chunks: Vec<(String, usize)> = lines
        .chunks(CHUNK_LINES)
        .map(|chunk| (chunk.join("\n"), chunk.len()))
        .collect();
    rsx! {
        div { class: "{BOX_ATOM_BASE}", style: "{style};min-width:max(100%, {widest}ch)",
            for (index, (text, count)) in chunks.iter().enumerate() {
                div { key: "{index}", class: "{BOX_ATOM_BASE}", "data-code-chunk": "{index}",
                    style: "content-visibility:auto;contain-intrinsic-block-size:auto {count}lh",
                    "{text}"
                }
            }
        }
    }
}

fn numbered_lines(code: &str, style: &str, wrap: bool) -> Element {
    let lines: Vec<&str> = code.split('\n').collect();
    let wrap_style = if wrap {
        "white-space:pre-wrap;word-break:break-all"
    } else {
        "white-space:pre"
    };
    rsx! {
        div { class: "{BOX_ATOM_BASE}", style: "{style}",
            for (index, line) in lines.iter().enumerate() {
                span { key: "{index}", "data-line": "{index}", style: "display:block;{wrap_style}",
                    span { style: "display:inline-block;min-width:2.5em;padding-right:1rem;text-align:right;user-select:none;opacity:0.5;font-variant-numeric:tabular-nums", "{index + 1}" }
                    "{line}"
                }
            }
        }
    }
}

#[allow(non_snake_case)]
fn Block(input: BlockProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut wrap = use_signal(|| get_bool_or(props, "wordWrap", false));
    let mut active_file = use_signal(|| 0usize);
    let relay: EventSink =
        Callback::new(
            move |(event, payload): (String, Value)| match event.as_str() {
                "__cb:wrap" => wrap.set(!wrap()),
                "__cb:tab" => {
                    if let Some(index) = get_string(&payload, "tabId").and_then(|id| {
                        id.strip_prefix("file-")
                            .and_then(|n| n.parse::<usize>().ok())
                    }) {
                        active_file.set(index);
                    }
                }
                _ => {}
            },
        );
    let local = view.for_source(view.source_trait.clone(), relay);
    let view = &local;
    let render = |spec: &Value| registry.render_spec(spec, None, view);

    let class_name = get_string_or(props, "className", "");
    if get_bool_or(props, "isLoading", false) {
        let spec = pattern_spec(
            "loading-state",
            [
                ("message", Value::string("Loading...")),
                ("className", Value::string(class_name.to_string())),
            ],
        );
        return render(&spec);
    }
    if let Some(error) = props
        .get("error")
        .filter(|error| error.as_object().is_some())
    {
        let spec = pattern_spec(
            "error-state",
            [
                ("title", Value::string("Code viewer error")),
                (
                    "message",
                    Value::string(get_string_or(error, "message", "")),
                ),
                ("className", Value::string(class_name.to_string())),
            ],
        );
        return render(&spec);
    }

    let language = get_string_or(props, "language", "text");
    let files = get_array(props, "files");
    let title = get_string(props, "title").filter(|t| !t.is_empty());
    let mode = get_string_or(props, "mode", "code");
    let editable = get_bool_or(props, "editable", false);
    let show_line_numbers = get_bool_or(props, "showLineNumbers", false);
    let actions = get_array(props, "actions");
    let diff = diff_rows(props);
    let is_viewer = title.is_some()
        || !files.is_empty()
        || (show_line_numbers && !editable)
        || diff.is_some()
        || mode == "diff"
        || !actions.is_empty();
    let effective_copy = props
        .get("showCopy")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| get_bool_or(props, "showCopyButton", true));
    let file = files.get(active_file());
    let active_language = file
        .and_then(|f| get_string(f, "language"))
        .unwrap_or_else(|| language.clone());
    let active_code = match file {
        Some(f) => get_string_or(f, "code", ""),
        None => get_string_or(props, "code", ""),
    };
    let dark = view
        .theme_mode
        .map(|mode| mode.as_str() == "dark")
        .unwrap_or(true);
    let viewer_color = if dark { "#d4d4d4" } else { "#383a42" };
    let region_label = title.clone().unwrap_or_else(|| "Code".to_string());
    let height = max_height(props);

    if is_viewer && active_code.is_empty() && diff.is_none() {
        let spec = pattern_spec(
            "empty-state",
            [
                ("icon", Value::string("code")),
                ("title", Value::string("No code")),
                ("description", Value::string("No code to display.")),
                ("className", Value::string(class_name.to_string())),
            ],
        );
        return render(&spec);
    }

    if is_viewer {
        let wrapping = wrap();
        let tab_strip = (files.len() > 1).then(|| {
            pattern_spec(
                "tabs",
                [
                    (
                        "tabs",
                        Value::Array(
                            files
                                .iter()
                                .enumerate()
                                .map(|(index, f)| {
                                    object_value([
                                        ("id", Value::string(format!("file-{index}"))),
                                        ("label", Value::string(get_string_or(f, "label", ""))),
                                    ])
                                })
                                .collect::<Vec<_>>()
                                .into(),
                        ),
                    ),
                    (
                        "activeTab",
                        Value::string(format!("file-{}", active_file())),
                    ),
                    ("tabChangeEvent", Value::string("__cb:tab")),
                ],
            )
        });
        let lead_icon = pattern_spec(
            "icon",
            [
                (
                    "name",
                    Value::string(if mode == "diff" { "file-text" } else { "code" }),
                ),
                ("size", Value::string("sm")),
                ("className", Value::string("text-muted-foreground")),
            ],
        );
        let title_spec = title.as_ref().map(|title| {
            pattern_spec(
                "typography",
                [
                    ("variant", Value::string("small")),
                    ("weight", Value::string("medium")),
                    ("className", Value::string("truncate")),
                    ("content", Value::string(title.to_string())),
                ],
            )
        });
        let language_badge =
            (!active_language.is_empty() && active_language != "text").then(|| {
                pattern_spec(
                    "badge",
                    [
                        ("label", Value::string(active_language.to_string())),
                        ("variant", Value::string("default")),
                    ],
                )
            });
        let wrap_button = pattern_spec(
            "button",
            [
                ("label", Value::string("")),
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("wrap-text")),
                ("aria-label", Value::string("Wrap lines")),
                ("aria-pressed", Value::string(wrapping.to_string())),
                (
                    "className",
                    Value::string(if wrapping { "text-primary" } else { "" }),
                ),
                ("action", Value::string("__cb:wrap")),
            ],
        );
        let copy_button = effective_copy.then(|| {
            pattern_spec(
                "button",
                [
                    ("label", Value::string("")),
                    ("variant", Value::string("ghost")),
                    ("size", Value::string("sm")),
                    ("icon", Value::string("copy")),
                    ("aria-label", Value::string("Copy code")),
                ],
            )
        });
        let action_badges: Vec<Value> = actions
            .iter()
            .map(|action| {
                let mut fields = vec![
                    ("label", Value::string(get_string_or(action, "label", ""))),
                    ("variant", Value::string("default")),
                    (
                        "className",
                        Value::string("cursor-pointer hover:opacity-80 transition-opacity"),
                    ),
                ];
                if let Some(event) = get_string(action, "event") {
                    fields.push(("action", Value::string(event)));
                }
                pattern_spec("badge", fields)
            })
            .collect();
        let body_style = if wrapping {
            "white-space:pre-wrap;word-break:break-all"
        } else {
            "white-space:pre;word-break:normal"
        };
        let code_style = format!(
            "margin:0;padding:0.125rem 1rem;{body_style};color:{viewer_color};font-family:{MONO_FONT};font-size:12px;line-height:1.6"
        );
        let numbered_style = format!(
            "margin:0;padding:0.25rem 0;color:{viewer_color};font-family:{MONO_FONT};font-size:12px;line-height:1.6;direction:ltr;text-align:start"
        );
        let diff_elements: Option<Vec<Element>> = diff.as_ref().map(|rows| {
            rows.iter()
                .enumerate()
                .map(|(index, row)| {
                    let number = typography_spec(
                        "caption",
                        Some("secondary"),
                        "w-8 text-right mr-3 select-none tabular-nums flex-shrink-0",
                        &row.line_number.map(|n| n.to_string()).unwrap_or_default(),
                    );
                    let text_wrap = if wrapping { "whitespace-pre-wrap break-all" } else { "whitespace-pre" };
                    let content = if row.content.is_empty() { " " } else { row.content.as_str() };
                    rsx! {
                        div { key: "{index}", class: "flex flex-row gap-0 items-start justify-start {row.bg} px-4 py-0.5",
                            if show_line_numbers { {render(&number)} }
                            div { class: "text-xs font-normal text-foreground font-mono flex-1 min-w-0 {row.text} {text_wrap}",
                                span { class: "{BOX_ATOM_BASE} select-none opacity-50 mr-2", "{row.prefix}" }
                                "{content}"
                            }
                        }
                    }
                })
                .collect()
        });
        let code_element = if show_line_numbers {
            numbered_lines(&active_code, &numbered_style, wrapping)
        } else {
            plain_chunks(&active_code, &code_style)
        };
        let class = format!("{CARD} overflow-hidden flex flex-col {class_name}");
        return rsx! {
            div { class: "{class}", style: "height:auto", ..shared_root_attrs(props),
                div { class: "{BOX_ATOM_BASE} flex flex-col flex-1 min-h-0",
                    if let Some(tabs) = tab_strip.as_ref() {
                        div { class: "{BOX_ATOM_BASE} border-b border-border", {render(tabs)} }
                    }
                    div { class: "flex flex-row gap-2 items-center justify-between px-4 py-2 border-b border-border bg-muted/30",
                        div { class: "flex flex-row gap-2 items-center",
                            {render(&lead_icon)}
                            if let Some(spec) = title_spec.as_ref() { {render(spec)} }
                            if let Some(spec) = language_badge.as_ref() { {render(spec)} }
                        }
                        div { class: "flex flex-row gap-1 items-center",
                            {render(&wrap_button)}
                            if let Some(spec) = copy_button.as_ref() { {render(spec)} }
                            for spec in action_badges.iter() { {render(spec)} }
                        }
                    }
                    div { class: "{BOX_ATOM_BASE} overflow-auto min-h-0 bg-muted/20", style: "max-height:{height}", dir: "ltr", role: "region", tabindex: "0", aria_label: "{region_label}",
                        if let Some(elements) = diff_elements {
                            div { style: "display:flex;flex-direction:column", class: "font-mono text-xs",
                                for element in elements { {element} }
                            }
                        } else {
                            div { class: "font-mono text-xs", {code_element} }
                        }
                    }
                }
            }
        };
    }

    let show_badge = get_bool_or(props, "showLanguageBadge", true);
    let copy_class = format!(
        "opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity text-muted-foreground hover:text-foreground{}",
        if show_badge { "" } else { " absolute top-2 end-2 z-10" }
    );
    let copy_button = effective_copy.then(|| {
        pattern_spec(
            "button",
            [
                ("label", Value::string("")),
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("copy")),
                ("className", Value::string(copy_class)),
                ("aria-label", Value::string("Copy")),
            ],
        )
    });
    let badge = pattern_spec(
        "badge",
        [
            ("label", Value::string(language.to_string())),
            ("variant", Value::string("default")),
            ("size", Value::string("sm")),
        ],
    );
    let code = get_string_or(props, "code", "");
    let box_radius = if show_badge {
        "0 0 0.5rem 0.5rem"
    } else {
        "0.5rem"
    };
    let code_style = "margin:0;white-space:pre;color:#d4d4d4";
    let class = format!("{BOX_ATOM_BASE} relative group not-prose {class_name}");
    rsx! {
        div { class: "{class}", style: "display:flex;flex-direction:column;height:100%", ..shared_root_attrs(props),
            if show_badge {
                div { class: "flex flex-row gap-2 items-center justify-between px-3 py-2 bg-[var(--color-card)] rounded-t-container border-b border-border", "data-testid": "code-block-header",
                    {render(&badge)}
                    if let Some(spec) = copy_button.as_ref() { {render(spec)} }
                }
            } else if let Some(spec) = copy_button.as_ref() {
                {render(spec)}
            }
            div { dir: "ltr", class: "[&_code]:!bg-transparent [&_code]:!p-0 [&_code]:!border-0 [&_code]:!shadow-none [&_span]:!bg-transparent",
                role: "region", tabindex: "0", aria_label: "{region_label}",
                style: "flex:1;min-height:0;overflow-x:auto;overflow-y:auto;max-height:{height};overscroll-behavior:auto;touch-action:pan-x pan-y;contain:paint;background-color:#1e1e1e;border-radius:{box_radius}",
                div { style: "padding:1rem", {plain_chunks(&code, code_style)} }
            }
        }
    }
}

impl DioxusPatternRenderer for CodeBlockRenderer {
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
        rsx! { Block { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
