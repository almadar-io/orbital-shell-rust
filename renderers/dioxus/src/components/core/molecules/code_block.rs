//! Native body of the `code-block` pattern, mirroring the web `CodeBlock` molecule (read-only
//! standard block, viewer mode with files / diff / line numbers). Syntax colouring is not carried:
//! no Rust highlighter is a dependency, so code paints as the web does before its highlighter loads.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::number_value;
use crate::renderer::{
    compute_line_diff, object_value, pattern_spec, typography_spec, DioxusPatternRenderer,
    DioxusView, EventSink, LineDiffKind, OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::html::ScrollBehavior;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::rc::Rc;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CaretShape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, PartialEq, Debug)]
struct BufferCell {
    col: usize,
    ch: char,
    caret: Option<CaretShape>,
    selected: bool,
    syntax: Option<&'static str>,
    annotation: Option<&'static str>,
}

const SYNTAX_TONES: [(&str, &str); 9] = [
    ("keyword", "--color-primary"),
    ("string", "--color-success"),
    ("comment", "--color-muted-foreground"),
    ("number", "--color-warning"),
    ("type", "--color-info"),
    ("function", "--color-accent"),
    ("operator", "--color-secondary"),
    ("punctuation", "--color-text-muted"),
    ("variable", "--color-foreground"),
];

const ANNOTATION_TONES: [(&str, &str); 6] = [
    ("search", "--color-primary"),
    ("review", "--color-secondary"),
    ("review-recent", "--color-accent"),
    ("error", "--color-error"),
    ("warning", "--color-warning"),
    ("info", "--color-info"),
];

fn syntax_tone(tone: &str) -> Option<&'static str> {
    SYNTAX_TONES
        .iter()
        .find(|(name, _)| *name == tone)
        .map(|(name, _)| *name)
}

fn annotation_tone(tone: &str) -> Option<&'static str> {
    ANNOTATION_TONES
        .iter()
        .find(|(name, _)| *name == tone)
        .map(|(name, _)| *name)
}

fn tone_token(table: &[(&'static str, &'static str)], tone: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(name, _)| *name == tone)
        .map(|(_, token)| *token)
}

fn mix(token: &str, percent: u8) -> String {
    format!("color-mix(in srgb, var({token}) {percent}%, transparent)")
}

fn position(props: &Value, row: &str, col: &str) -> Option<(usize, usize)> {
    Some((
        get_number(props, row).filter(|n| *n >= 0.0)? as usize,
        get_number(props, col).filter(|n| *n >= 0.0)? as usize,
    ))
}

fn caret_shape(props: &Value) -> CaretShape {
    match get_string(props, "caret").as_deref() {
        Some("bar") => CaretShape::Bar,
        Some("underline") => CaretShape::Underline,
        _ => CaretShape::Block,
    }
}

fn buffer_lines(props: &Value) -> Vec<Vec<BufferCell>> {
    let code = get_string_or(props, "code", "");
    let cursor = props.get("cursor").and_then(|c| position(c, "row", "col"));
    let shape = caret_shape(props);
    let selection = props
        .get("selection")
        .filter(|s| s.as_object().is_some())
        .and_then(|s| {
            let from = position(s, "fromRow", "fromCol")?;
            let to = position(s, "toRow", "toCol")?;
            Some((
                from.min(to),
                from.max(to),
                get_bool_or(s, "linewise", false),
            ))
        });
    let decorations: Vec<((usize, usize), (usize, usize), &'static str, bool)> =
        get_array(props, "decorations")
            .iter()
            .filter_map(|d| {
                let from = position(d, "fromRow", "fromCol")?;
                let to = position(d, "toRow", "toCol")?;
                let tone = get_string(d, "tone")?;
                match syntax_tone(&tone) {
                    Some(known) => Some((from, to, known, true)),
                    None => annotation_tone(&tone).map(|known| (from, to, known, false)),
                }
            })
            .collect();
    code.split('\n')
        .enumerate()
        .map(|(row, line)| {
            let chars: Vec<char> = line.chars().collect();
            chars
                .into_iter()
                .chain(std::iter::once(' '))
                .enumerate()
                .map(|(col, ch)| {
                    let at = (row, col);
                    let mut cell = BufferCell {
                        col,
                        ch,
                        caret: (cursor == Some(at)).then_some(shape),
                        selected: selection.is_some_and(|(min, max, linewise)| {
                            if linewise {
                                row >= min.0 && row <= max.0
                            } else {
                                at >= min && at <= max
                            }
                        }),
                        syntax: None,
                        annotation: None,
                    };
                    for (from, to, tone, is_syntax) in &decorations {
                        if at >= *from && at < *to {
                            if *is_syntax {
                                cell.syntax = Some(*tone);
                            } else {
                                cell.annotation = Some(*tone);
                            }
                        }
                    }
                    cell
                })
                .collect()
        })
        .collect()
}

fn cell_style(cell: &BufferCell) -> String {
    let mut style = String::new();
    if let Some(token) = cell.syntax.and_then(|tone| tone_token(&SYNTAX_TONES, tone)) {
        style.push_str(&format!("color:var({token});"));
    }
    if let Some(token) = cell
        .annotation
        .and_then(|tone| tone_token(&ANNOTATION_TONES, tone))
    {
        style.push_str(&format!("background-color:{};", mix(token, 35)));
    }
    if cell.selected {
        style.push_str(&format!("background-color:{};", mix("--color-primary", 40)));
    }
    match cell.caret {
        Some(CaretShape::Block) => style
            .push_str("background-color:var(--color-foreground);color:var(--color-background);"),
        Some(CaretShape::Bar) => {
            style.push_str("border-left:2px solid var(--color-foreground);margin-left:-2px;")
        }
        Some(CaretShape::Underline) => {
            style.push_str("border-bottom:2px solid var(--color-foreground);")
        }
        None => {}
    }
    style
}

fn caret_name(shape: CaretShape) -> &'static str {
    match shape {
        CaretShape::Block => "block",
        CaretShape::Bar => "bar",
        CaretShape::Underline => "underline",
    }
}

fn gutter_labels(count: usize, cursor_row: Option<usize>, relative: bool) -> Vec<String> {
    (0..count)
        .map(|row| match (relative, cursor_row) {
            (true, Some(at)) if row != at => row.abs_diff(at).to_string(),
            _ => (row + 1).to_string(),
        })
        .collect()
}

struct PopoverModel {
    from: (usize, usize),
    title: String,
    sections: Vec<(String, Vec<String>)>,
    busy: bool,
}

fn popover_model(props: &Value) -> Option<PopoverModel> {
    let popover = props.get("popover").filter(|p| p.as_object().is_some())?;
    Some(PopoverModel {
        from: position(popover, "fromRow", "fromCol")?,
        title: get_string_or(popover, "title", ""),
        sections: get_array(popover, "sections")
            .iter()
            .map(|section| {
                (
                    get_string_or(section, "heading", ""),
                    get_string_array(section, "lines"),
                )
            })
            .collect(),
        busy: get_bool_or(popover, "busy", false),
    })
}

fn emit_to(sink: Option<EventSink>, event: &Option<String>, payload: Value) {
    if let (Some(sink), Some(event)) = (sink, event) {
        sink.call((event.clone(), payload));
    }
}

fn cell_payload(editor_id: &str, row: usize, col: usize) -> Value {
    object_value([
        ("editorId", Value::string(editor_id)),
        ("row", number_value(row as f64)),
        ("col", number_value(col as f64)),
    ])
}

fn buffer_surface(
    props: &Value,
    sink: Option<EventSink>,
    mut surface: Signal<Option<Rc<MountedData>>>,
    mut hovered: Signal<Option<(usize, usize)>>,
) -> Element {
    let lines = buffer_lines(props);
    let cursor_row = props
        .get("cursor")
        .and_then(|c| position(c, "row", "col"))
        .map(|(row, _)| row);
    let editor_id = get_string_or(props, "editorId", "");
    let on_key = get_string(props, "onKey");
    let on_down = get_string(props, "onPointerDown");
    let on_hover = get_string(props, "onHover");
    let on_focus = get_string_or(props, "onEditorFocus", "EDITOR_FOCUS");
    let on_blur = get_string_or(props, "onEditorBlur", "EDITOR_BLUR");
    let relative = get_bool_or(props, "relativeLineNumbers", false);
    let gutter = (get_bool_or(props, "lineNumbers", false) || relative)
        .then(|| gutter_labels(lines.len(), cursor_row, relative));
    let popover = popover_model(props);
    let last_row = lines.len().saturating_sub(1);
    let last_len = lines
        .last()
        .map(|line| line.len().saturating_sub(1))
        .unwrap_or(0);
    let class_name = get_string_or(props, "className", "");
    let height = max_height(props);
    let label = get_string_or(props, "title", &crate::i18n::t("codeBlock.region"));

    let popover_top = popover.as_ref().map(|note| (note.from.0 + 1) as f64 * 1.6);
    let popover_left = popover.as_ref().map(|note| note.from.1);
    let focus_surface = Callback::new(move |_: ()| {
        if let Some(handle) = surface.peek().clone() {
            spawn(async move {
                if let Err(error) = handle.set_focus(true).await {
                    tracing::error!(%error, "code buffer focus");
                }
            });
        }
    });
    let press = {
        let editor_id = editor_id.clone();
        let on_down = on_down.clone();
        Callback::new(move |(row, col): (usize, usize)| {
            focus_surface.call(());
            emit_to(sink, &on_down, cell_payload(&editor_id, row, col));
        })
    };
    let hover = {
        let editor_id = editor_id.clone();
        Callback::new(move |(row, col): (usize, usize)| {
            if *hovered.peek() == Some((row, col)) {
                return;
            }
            hovered.set(Some((row, col)));
            emit_to(sink, &on_hover, cell_payload(&editor_id, row, col));
        })
    };
    let key_editor = editor_id.clone();
    let focus_editor = editor_id.clone();
    let blur_editor = editor_id.clone();
    let focus_event = Some(on_focus);
    let blur_event = Some(on_blur);

    let class = format!("{CARD} overflow-hidden {class_name}");
    rsx! {
        div { class: "{class}", style: "height:auto", ..shared_root_attrs(props),
            div {
                class: "{BOX_ATOM_BASE} overflow-auto flex flex-row",
                style: "max-height:{height};font-family:{MONO_FONT};font-size:12px;line-height:1.6;direction:ltr;outline:none",
                role: "textbox", tabindex: "0", aria_multiline: "true", aria_label: "{label}", "data-code-buffer": "true",
                onmounted: move |event| surface.set(Some(event.data())),
                onfocus: move |_| emit_to(sink, &focus_event, object_value([("editorId", Value::string(focus_editor.as_str()))])),
                onblur: move |_| emit_to(sink, &blur_event, object_value([("editorId", Value::string(blur_editor.as_str()))])),
                onkeydown: move |event| {
                    if event.is_composing() {
                        return;
                    }
                    event.prevent_default();
                    let modifiers = event.modifiers();
                    emit_to(sink, &on_key, object_value([
                        ("editorId", Value::string(key_editor.as_str())),
                        ("key", Value::string(event.key().to_string())),
                        ("code", Value::string(event.code().to_string())),
                        ("ctrl", Value::Bool(modifiers.ctrl())),
                        ("alt", Value::Bool(modifiers.alt())),
                        ("shift", Value::Bool(modifiers.shift())),
                        ("meta", Value::Bool(modifiers.meta())),
                        ("repeat", Value::Bool(event.is_auto_repeating())),
                    ]));
                },
                onmousedown: move |_| press.call((last_row, last_len)),
                if let Some(labels) = gutter.as_ref() {
                    div { class: "{BOX_ATOM_BASE} flex-shrink-0 select-none text-muted-foreground", style: "text-align:right;padding:0 1rem 0 0.5rem;font-variant-numeric:tabular-nums", "data-code-gutter": "true",
                        for (row, text) in labels.iter().enumerate() {
                            div { key: "{row}", style: "height:1.6em", "data-gutter-row": "{row}",
                                onmousedown: move |event| event.stop_propagation(),
                                "{text}"
                            }
                        }
                    }
                }
                div { style: "position:relative;flex:1;min-width:max-content;white-space:pre",
                    for (row, cells) in lines.iter().enumerate() {
                        div {
                            key: "{row}-{cursor_row == Some(row)}",
                            style: "height:1.6em;white-space:pre", "data-row": "{row}",
                            onmounted: move |event| {
                                if cursor_row == Some(row) {
                                    let handle = event.data();
                                    spawn(async move {
                                        if let Err(error) = handle.scroll_to(ScrollBehavior::Instant).await {
                                            tracing::error!(%error, "code buffer scroll");
                                        }
                                    });
                                }
                            },
                            onmousedown: {
                                let end = cells.len() - 1;
                                move |event| {
                                    event.stop_propagation();
                                    press.call((row, end));
                                }
                            },
                            for cell in cells.iter() {
                                span {
                                    key: "{cell.col}",
                                    style: "{cell_style(cell)}",
                                    "data-row": "{row}", "data-col": "{cell.col}",
                                    "data-caret": cell.caret.map(caret_name),
                                    "data-selected": cell.selected.then_some("true"),
                                    "data-syntax": cell.syntax,
                                    "data-annotation": cell.annotation,
                                    onmousedown: {
                                        let col = cell.col;
                                        move |event| {
                                            event.stop_propagation();
                                            press.call((row, col));
                                        }
                                    },
                                    onmousemove: {
                                        let col = cell.col;
                                        move |_| hover.call((row, col))
                                    },
                                    "{cell.ch}"
                                }
                            }
                        }
                    }
                    if let Some(note) = popover.as_ref() {
                        div {
                            class: "surface-content rounded-container shadow-lg border border-border p-card-sm",
                            style: "position:absolute;z-index:50;top:{popover_top.unwrap_or(0.0)}em;left:{popover_left.unwrap_or(0)}ch;min-width:16ch;max-width:48ch;white-space:normal;pointer-events:none",
                            role: "note", "data-code-popover": "true",
                            div { class: "font-bold text-foreground", "{note.title}" }
                            for (index, (heading, body)) in note.sections.iter().enumerate() {
                                div { key: "{index}", "data-popover-section": "{index}",
                                    div { class: "text-muted-foreground", "{heading}" }
                                    for (at, line) in body.iter().enumerate() {
                                        div { key: "{at}", class: "text-foreground", "{line}" }
                                    }
                                }
                            }
                            if note.busy {
                                div { class: "text-muted-foreground", "data-popover-busy": "true", "..." }
                            }
                        }
                    }
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
    let surface = use_signal(|| None::<Rc<MountedData>>);
    let hovered = use_signal(|| None::<(usize, usize)>);
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
                ("message", Value::string(crate::i18n::t("common.loading"))),
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
                (
                    "title",
                    Value::string(crate::i18n::t("display.codeViewerError")),
                ),
                (
                    "message",
                    Value::string(get_string_or(error, "message", "")),
                ),
                ("className", Value::string(class_name.to_string())),
            ],
        );
        return render(&spec);
    }

    if props
        .get("cursor")
        .is_some_and(|cursor| cursor.as_object().is_some())
    {
        let sink = view.sink;
        return buffer_surface(props, sink, surface, hovered);
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
        || props
            .get("actions")
            .is_some_and(|authored| !authored.is_null());
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
    let region_label = title
        .clone()
        .unwrap_or_else(|| crate::i18n::t("codeBlock.region"));
    let height = max_height(props);

    if is_viewer && active_code.is_empty() && diff.is_none() {
        let spec = pattern_spec(
            "empty-state",
            [
                ("icon", Value::string("code")),
                ("title", Value::string(crate::i18n::t("display.noCode"))),
                (
                    "description",
                    Value::string(crate::i18n::t("codeBlock.noCode")),
                ),
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
                (
                    "aria-label",
                    Value::string(crate::i18n::t("aria.wrapLines")),
                ),
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
                    ("aria-label", Value::string(crate::i18n::t("aria.copyCode"))),
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
                ("aria-label", Value::string(crate::i18n::t("common.copy"))),
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

#[cfg(test)]
mod buffer_tests {
    use super::*;

    fn props(json: serde_json::Value) -> Value {
        serde_json::from_value(json).unwrap()
    }

    fn painted(lines: &[Vec<BufferCell>], row: usize, col: usize) -> &BufferCell {
        &lines[row][col]
    }

    #[test]
    fn buffer_caret_block_covers_the_character_at_col() {
        let lines = buffer_lines(&props(
            serde_json::json!({"code": "ab\ncde", "cursor": {"row": 1, "col": 2}, "caret": "block"}),
        ));
        let cell = painted(&lines, 1, 2);
        assert_eq!((cell.ch, cell.caret), ('e', Some(CaretShape::Block)));
        assert!(cell_style(cell).contains("background-color:var(--color-foreground)"));
        let carets = lines.iter().flatten().filter(|c| c.caret.is_some()).count();
        assert_eq!(carets, 1);
    }

    #[test]
    fn buffer_caret_absent_shape_is_block_and_bar_underline_follow_the_prop() {
        let base = |caret: serde_json::Value| {
            let mut json = serde_json::json!({"code": "ab\ncde", "cursor": {"row": 1, "col": 2}});
            json["caret"] = caret;
            buffer_lines(&props(json))
        };
        assert_eq!(
            base(serde_json::Value::Null)[1][2].caret,
            Some(CaretShape::Block)
        );
        let bar = base(serde_json::json!("bar"));
        assert_eq!(
            (bar[1][2].ch, bar[1][2].caret),
            ('e', Some(CaretShape::Bar))
        );
        assert!(cell_style(&bar[1][2]).contains("border-left"));
        let underline = base(serde_json::json!("underline"));
        assert!(cell_style(&underline[1][2]).contains("border-bottom"));
        assert!(!cell_style(&underline[1][2]).contains("border-left"));
    }

    #[test]
    fn buffer_caret_at_line_end_is_a_space_width_block_after_the_last_char() {
        let lines = buffer_lines(&props(
            serde_json::json!({"code": "ab\ncde", "cursor": {"row": 0, "col": 2}, "caret": "block"}),
        ));
        assert_eq!(lines[0].len(), 3);
        assert_eq!(
            (lines[0][2].ch, lines[0][2].caret),
            (' ', Some(CaretShape::Block))
        );
        assert_eq!(lines[0][1].caret, None);
    }

    #[test]
    fn buffer_selection_linewise_paints_every_cell_of_both_rows() {
        let lines = buffer_lines(&props(serde_json::json!({
            "code": "ab\ncd\nef", "cursor": {"row": 1, "col": 0},
            "selection": {"fromRow": 1, "fromCol": 1, "toRow": 0, "toCol": 0, "linewise": true}
        })));
        assert!(lines[0].iter().all(|c| c.selected));
        assert!(lines[1].iter().all(|c| c.selected));
        assert!(lines[2].iter().all(|c| !c.selected));
    }

    #[test]
    fn buffer_selection_charwise_is_inclusive_of_the_max_end_and_null_paints_none() {
        let lines = buffer_lines(&props(serde_json::json!({
            "code": "abcd", "cursor": {"row": 0, "col": 2},
            "selection": {"fromRow": 0, "fromCol": 1, "toRow": 0, "toCol": 2, "linewise": false}
        })));
        let selected: String = lines[0]
            .iter()
            .filter(|c| c.selected)
            .map(|c| c.ch)
            .collect();
        assert_eq!(selected, "bc");
        let none = buffer_lines(&props(
            serde_json::json!({"code": "abcd", "cursor": {"row": 0, "col": 0}, "selection": null}),
        ));
        assert!(none[0].iter().all(|c| !c.selected));
    }

    #[test]
    fn buffer_decorations_syntax_colors_text_annotation_paints_background() {
        let lines = buffer_lines(&props(serde_json::json!({
            "code": "fun x", "cursor": {"row": 0, "col": 5},
            "decorations": [
                {"id": "k", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 3, "tone": "keyword"},
                {"id": "r", "fromRow": 0, "fromCol": 4, "toRow": 0, "toCol": 5, "tone": "review"},
                {"id": "u", "fromRow": 0, "fromCol": 3, "toRow": 0, "toCol": 4, "tone": "no-such-tone"}
            ]
        })));
        for col in 0..3 {
            let style = cell_style(&lines[0][col]);
            assert!(style.contains("color:var(--color-primary)"), "{style}");
            assert!(!style.contains("background-color"), "{style}");
        }
        let review = cell_style(&lines[0][4]);
        assert!(
            review.contains("background-color:color-mix")
                && review.contains("var(--color-secondary)"),
            "{review}"
        );
        assert!(!review.contains(";color:"), "{review}");
        assert_eq!(cell_style(&lines[0][3]), "");
    }

    #[test]
    fn buffer_later_decoration_paints_over_earlier() {
        let lines = buffer_lines(&props(serde_json::json!({
            "code": "ab", "cursor": {"row": 0, "col": 0},
            "decorations": [
                {"id": "1", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 2, "tone": "keyword"},
                {"id": "2", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 1, "tone": "string"}
            ]
        })));
        assert_eq!(lines[0][0].syntax, Some("string"));
        assert_eq!(lines[0][1].syntax, Some("keyword"));
    }

    #[test]
    fn buffer_popover_model_carries_title_sections_busy_and_null_is_none() {
        let note = popover_model(&props(serde_json::json!({
            "popover": {"fromRow": 1, "fromCol": 2, "toRow": 1, "toCol": 4, "title": "Note",
                "sections": [{"heading": "Why", "lines": ["a", "b"]}], "busy": true}
        })))
        .unwrap();
        assert_eq!(
            (note.from, note.title.as_str(), note.busy),
            ((1, 2), "Note", true)
        );
        assert_eq!(
            note.sections,
            vec![("Why".to_string(), vec!["a".to_string(), "b".to_string()])]
        );
        assert!(popover_model(&props(serde_json::json!({"popover": null}))).is_none());
        assert!(popover_model(&props(serde_json::json!({}))).is_none());
    }

    #[test]
    fn buffer_relative_gutter_shows_distance_and_absolute_on_the_cursor_row() {
        assert_eq!(gutter_labels(5, Some(2), true), ["2", "1", "3", "1", "2"]);
        assert_eq!(gutter_labels(3, Some(1), false), ["1", "2", "3"]);
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod buffer_dom_tests {
    use super::*;
    use crate::renderer::DioxusView;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_traits::events::{BlitzKeyEvent, KeyState, MouseEventButtons, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_native::DioxusDocument;
    use keyboard_types::{Code, Key, Location, Modifiers};
    use orbital_core::ValueMap;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static EVENTS: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
        static PROPS: RefCell<Option<Value>> = const { RefCell::new(None) };
    }

    fn app() -> Element {
        let props = PROPS.with(|p| p.borrow().clone().unwrap());
        let entities = HashMap::new();
        let trait_content = ValueMap::new();
        let sink = Callback::new(|event| EVENTS.with(|events| events.borrow_mut().push(event)));
        let view = DioxusView {
            trait_name: "Code",
            orbital_name: "Code",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        let mut registry = DioxusPatternRegistry::new();
        registry.register(Box::new(CodeBlockRenderer));
        registry.render_spec(&props, None, &view)
    }

    fn mount(json: serde_json::Value) -> DioxusDocument {
        EVENTS.with(|events| events.borrow_mut().clear());
        PROPS.with(|p| *p.borrow_mut() = Some(serde_json::from_value(json).unwrap()));
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig {
                viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
                ..Default::default()
            },
        );
        doc.initial_build();
        settle(&mut doc);
        doc
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..4 {
            if !doc.poll(None) {
                break;
            }
        }
        doc.resolve(0.0);
    }

    fn mouse_at(doc: &mut DioxusDocument, selector: &str, down: bool) {
        let id = doc.query_selector(selector).unwrap().unwrap();
        let mut mouse = doc
            .get_node(id)
            .unwrap()
            .synthetic_click_event_data(Modifiers::default());
        mouse.buttons = MouseEventButtons::None;
        if down {
            mouse.buttons = MouseEventButtons::Primary;
            doc.handle_ui_event(UiEvent::MouseDown(mouse));
        } else {
            doc.handle_ui_event(UiEvent::MouseMove(mouse));
        }
        settle(doc);
    }

    fn events() -> Vec<(String, Value)> {
        EVENTS.with(|events| events.borrow().clone())
    }

    fn buffer(extra: serde_json::Value) -> serde_json::Value {
        let mut json = serde_json::json!({
            "type": "code-block", "code": "ab\ncde", "editorId": "ed1",
            "cursor": {"row": 1, "col": 2},
            "onKey": "KEY", "onPointerDown": "DOWN", "onHover": "HOVER"
        });
        for (key, value) in extra.as_object().unwrap() {
            json[key] = value.clone();
        }
        json
    }

    #[test]
    fn buffer_dom_mode_is_off_without_cursor() {
        let mut off = mount(serde_json::json!({"type": "code-block", "code": "ab\ncde"}));
        assert!(off.query_selector("[data-code-buffer]").unwrap().is_none());
        let mut on = mount(buffer(serde_json::json!({})));
        assert!(on.query_selector("[data-code-buffer]").unwrap().is_some());
        assert!(on
            .query_selector("[data-row='1'][data-col='2'][data-caret=block]")
            .unwrap()
            .is_some());
    }

    #[test]
    fn buffer_dom_popover_renders_title_sections_busy_and_null_renders_nothing() {
        let note = serde_json::json!({"popover": {"fromRow": 0, "fromCol": 1, "toRow": 0, "toCol": 2,
            "title": "Hint", "sections": [{"heading": "Why", "lines": ["line one"]}], "busy": true}});
        let mut doc = mount(buffer(note));
        assert!(doc.query_selector("[data-code-popover]").unwrap().is_some());
        assert!(doc
            .query_selector("[data-popover-section='0']")
            .unwrap()
            .is_some());
        assert!(doc.query_selector("[data-popover-busy]").unwrap().is_some());
        let mut idle = mount(buffer(serde_json::json!({"popover": null})));
        assert!(idle
            .query_selector("[data-code-popover]")
            .unwrap()
            .is_none());
    }

    #[test]
    fn buffer_dom_relative_gutter_labels() {
        let mut doc = mount(buffer(serde_json::json!({
            "code": "a\nb\nc\nd\ne", "cursor": {"row": 2, "col": 0},
            "lineNumbers": true, "relativeLineNumbers": true
        })));
        let mut labels = Vec::new();
        for row in 0..5 {
            let id = doc
                .query_selector(&format!("[data-gutter-row='{row}']"))
                .unwrap()
                .unwrap();
            labels.push(doc.get_node(id).unwrap().text_content());
        }
        assert_eq!(labels, ["2", "1", "3", "1", "2"]);
    }

    #[test]
    fn buffer_dom_keydown_emits_on_key_with_every_field() {
        let mut doc = mount(buffer(serde_json::json!({})));
        let surface = doc.query_selector("[data-code-buffer]").unwrap().unwrap();
        doc.set_focus_to(surface);
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Character("x".into()),
            code: Code::KeyX,
            modifiers: Modifiers::CONTROL | Modifiers::SHIFT,
            location: Location::Standard,
            is_auto_repeating: true,
            is_composing: false,
            state: KeyState::Pressed,
            text: None,
        }));
        settle(&mut doc);
        let seen = events();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "KEY");
        let payload = &seen[0].1;
        assert_eq!(get_string(payload, "editorId").as_deref(), Some("ed1"));
        assert_eq!(get_string(payload, "key").as_deref(), Some("x"));
        assert_eq!(get_string(payload, "code").as_deref(), Some("KeyX"));
        for (field, expected) in [
            ("ctrl", true),
            ("alt", false),
            ("shift", true),
            ("meta", false),
            ("repeat", true),
        ] {
            assert_eq!(get_bool(payload, field), Some(expected), "{field}");
        }
    }

    #[test]
    fn buffer_dom_composing_keydown_emits_nothing() {
        let mut doc = mount(buffer(serde_json::json!({})));
        let surface = doc.query_selector("[data-code-buffer]").unwrap().unwrap();
        doc.set_focus_to(surface);
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Character("x".into()),
            code: Code::KeyX,
            modifiers: Modifiers::default(),
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: true,
            state: KeyState::Pressed,
            text: None,
        }));
        settle(&mut doc);
        assert!(events().is_empty());
    }

    #[test]
    fn buffer_dom_press_emits_on_pointer_down_and_hover_once_per_cell() {
        let mut doc = mount(buffer(serde_json::json!({})));
        mouse_at(&mut doc, "[data-row='1'][data-col='1']", true);
        let seen = events();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "DOWN");
        assert_eq!(get_number(&seen[0].1, "row"), Some(1.0));
        assert_eq!(get_number(&seen[0].1, "col"), Some(1.0));
        assert_eq!(get_string(&seen[0].1, "editorId").as_deref(), Some("ed1"));

        EVENTS.with(|events| events.borrow_mut().clear());
        mouse_at(&mut doc, "[data-row='0'][data-col='1']", false);
        mouse_at(&mut doc, "[data-row='0'][data-col='1']", false);
        let hovers: Vec<_> = events()
            .into_iter()
            .filter(|(name, _)| name == "HOVER")
            .collect();
        assert_eq!(hovers.len(), 1);
        assert_eq!(get_number(&hovers[0].1, "row"), Some(0.0));
        assert_eq!(get_number(&hovers[0].1, "col"), Some(1.0));
    }
}
