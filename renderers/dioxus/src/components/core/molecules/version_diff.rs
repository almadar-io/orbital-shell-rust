//! Native body of the `version-diff` pattern, mirroring the web `VersionDiff` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    compute_line_diff, object_value, pattern_spec, typography_spec, DioxusPatternRenderer,
    DioxusView, EventSink, LineDiffKind, LineDiffLine, OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::HashSet;

/// Dioxus renderer for the `version-diff` pattern.
pub struct VersionDiffRenderer;

impl VersionDiffRenderer {
    const NAME: &str = "version-diff";
}

const CONTEXT_LINES: usize = 3;
const MIN_HIDDEN: usize = 4;
const CARD: &str = "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md";

#[derive(Props, Clone, PartialEq)]
struct DiffProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

struct Revision {
    id: String,
    label: String,
    author: Option<String>,
    timestamp: Option<String>,
    content: String,
}

#[derive(Clone)]
enum Row {
    Header(String),
    Line(LineDiffLine, bool),
    Gap { id: usize, count: usize },
}

fn parse_revisions(props: &Value) -> Vec<Revision> {
    get_array(props, "revisions")
        .iter()
        .filter_map(|value| {
            Some(Revision {
                id: get_string(value, "id")?,
                label: get_string(value, "label")?,
                content: get_string(value, "content")?,
                author: get_string(value, "author"),
                timestamp: get_string(value, "timestamp"),
            })
        })
        .collect()
}

fn parse_kind(value: &Value) -> LineDiffKind {
    match get_string(value, "type").as_deref() {
        Some("added") => LineDiffKind::Added,
        Some("removed") => LineDiffKind::Removed,
        _ => LineDiffKind::Unchanged,
    }
}

fn parse_context(value: &Value) -> bool {
    get_string(value, "type").as_deref() == Some("context")
}

fn line_number(value: &Value, key: &str) -> Option<usize> {
    get_number(value, key).map(|number| number as usize)
}

fn collapse_unchanged(rows: Vec<Row>, expanded: &HashSet<usize>) -> Vec<Row> {
    let unchanged =
        |row: &Row| matches!(row, Row::Line(line, false) if line.kind == LineDiffKind::Unchanged);
    let is_line = |row: Option<&Row>| matches!(row, Some(Row::Line(..)));
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        if !unchanged(&rows[i]) {
            out.push(rows[i].clone());
            i += 1;
            continue;
        }
        let mut j = i;
        while j < rows.len() && unchanged(&rows[j]) {
            j += 1;
        }
        let keep_head = if i > 0 && is_line(rows.get(i - 1)) {
            CONTEXT_LINES
        } else {
            0
        };
        let keep_tail = if is_line(rows.get(j)) {
            CONTEXT_LINES
        } else {
            0
        };
        let hidden = (j - i).saturating_sub(keep_head + keep_tail);
        let id = i + keep_head;
        if hidden < MIN_HIDDEN || expanded.contains(&id) {
            out.extend(rows[i..j].iter().cloned());
        } else {
            out.extend(rows[i..i + keep_head].iter().cloned());
            out.push(Row::Gap { id, count: hidden });
            out.extend(rows[j - keep_tail..j].iter().cloned());
        }
        i = j;
    }
    out
}

fn number_text(value: Option<usize>) -> String {
    value.map(|number| number.to_string()).unwrap_or_default()
}

fn nonblank(content: &str) -> &str {
    if content.is_empty() {
        " "
    } else {
        content
    }
}

fn emit(sink: Option<EventSink>, event: &Option<String>, id: &str) {
    if let (Some(sink), Some(event)) = (sink, event) {
        sink.call((event.clone(), object_value([("id", Value::string(id))])));
    }
}

fn line_number_cell(value: Option<usize>, margin: &str) -> Value {
    typography_spec(
        "caption",
        Some("secondary"),
        &format!("w-8 text-right {margin} select-none tabular-nums flex-shrink-0"),
        &number_text(value),
    )
}

#[allow(non_snake_case)]
fn Diff(input: DiffProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let revisions = parse_revisions(props);
    let hunks = props.get("hunks").and_then(Value::as_array);
    let fallback_before = revisions.first().map(|r| r.id.clone()).unwrap_or_default();
    let fallback_after = revisions
        .get(1)
        .or_else(|| revisions.first())
        .map(|r| r.id.clone())
        .unwrap_or_default();
    let mut internal_before =
        use_signal(|| get_string(props, "beforeId").unwrap_or(fallback_before.clone()));
    let mut internal_after =
        use_signal(|| get_string(props, "afterId").unwrap_or(fallback_after.clone()));
    let mut active_view = use_signal(|| get_string_or(props, "view", "side-by-side"));
    let mut expanded = use_signal(HashSet::<usize>::new);

    let active_before_id = get_string(props, "beforeId").unwrap_or_else(|| internal_before());
    let active_after_id = get_string(props, "afterId").unwrap_or_else(|| internal_after());
    let before_rev = revisions
        .iter()
        .find(|r| r.id == active_before_id)
        .or_else(|| revisions.first());
    let after_rev = revisions
        .iter()
        .find(|r| r.id == active_after_id)
        .or_else(|| revisions.get(1))
        .or_else(|| revisions.first());

    let flagged: Vec<Row> = match hunks {
        Some(hunks) => hunks
            .iter()
            .flat_map(|hunk| {
                let header = std::iter::once(Row::Header(get_string_or(hunk, "header", "")));
                let lines = get_array(hunk, "lines")
                    .into_iter()
                    .map(|line| {
                        let context = parse_context(&line);
                        Row::Line(
                            LineDiffLine {
                                kind: parse_kind(&line),
                                before_line_number: line_number(&line, "beforeLineNumber"),
                                after_line_number: line_number(&line, "afterLineNumber"),
                                content: get_string_or(&line, "content", ""),
                            },
                            context,
                        )
                    })
                    .collect::<Vec<_>>();
                header.chain(lines)
            })
            .collect(),
        None => compute_line_diff(
            before_rev.map(|r| r.content.as_str()).unwrap_or(""),
            after_rev.map(|r| r.content.as_str()).unwrap_or(""),
        )
        .into_iter()
        .map(|line| Row::Line(line, false))
        .collect(),
    };
    let (mut added, mut removed) = (0usize, 0usize);
    for row in &flagged {
        if let Row::Line(line, _) = row {
            match line.kind {
                LineDiffKind::Added => added += 1,
                LineDiffKind::Removed => removed += 1,
                LineDiffKind::Unchanged => {}
            }
        }
    }
    let shown = collapse_unchanged(flagged, &expanded.read());

    let sink = view.sink;
    let on_select_before = get_string(props, "onSelectBefore");
    let on_select_after = get_string(props, "onSelectAfter");
    let select_before_event = get_string(props, "selectBeforeEvent");
    let select_after_event = get_string(props, "selectAfterEvent");
    let on_revert = get_string(props, "onRevert").filter(|name| !name.is_empty());
    let revert_event = get_string(props, "revertEvent").filter(|name| !name.is_empty());
    let before_id_for_revert = before_rev.map(|r| r.id.clone());
    let relay: EventSink =
        Callback::new(
            move |(event, payload): (String, Value)| match event.as_str() {
                "__vd:before" => {
                    if let Some(id) = get_string(&payload, "value") {
                        internal_before.set(id.clone());
                        emit(sink, &on_select_before, &id);
                        emit(sink, &select_before_event, &id);
                    }
                }
                "__vd:after" => {
                    if let Some(id) = get_string(&payload, "value") {
                        internal_after.set(id.clone());
                        emit(sink, &on_select_after, &id);
                        emit(sink, &select_after_event, &id);
                    }
                }
                "__vd:view" => {
                    let next = if active_view() == "side-by-side" {
                        "inline"
                    } else {
                        "side-by-side"
                    };
                    active_view.set(next.to_string());
                }
                "__vd:revert" => {
                    if let Some(id) = &before_id_for_revert {
                        emit(sink, &on_revert, id);
                        emit(sink, &revert_event, id);
                    }
                }
                "__vd:gap" => {
                    if let Some(id) = get_number(&payload, "id") {
                        let mut next = expanded.peek().clone();
                        next.insert(id as usize);
                        expanded.set(next);
                    }
                }
                _ => {}
            },
        );
    let local = view.for_source(view.source_trait.clone(), relay);
    let view = &local;

    let options = Value::Array(
        revisions
            .iter()
            .map(|r| {
                object_value([
                    ("value", Value::string(r.id.to_string())),
                    ("label", Value::string(r.label.to_string())),
                ])
            })
            .collect::<Vec<_>>()
            .into(),
    );
    let side = active_view() == "side-by-side";
    let render = |spec: &Value| registry.render_spec(spec, None, view);

    let git = pattern_spec(
        "icon",
        [
            ("name", Value::string("git-commit")),
            ("size", Value::string("sm")),
            ("className", Value::string("text-muted-foreground")),
        ],
    );
    let compare = pattern_spec(
        "typography",
        [
            ("variant", Value::string("small")),
            ("weight", Value::string("medium")),
            ("className", Value::string("whitespace-nowrap")),
            ("content", Value::string("Compare")),
        ],
    );
    let to = typography_spec("caption", Some("secondary"), "", "to");
    let select = |value: &str, event: &str, label: &str| {
        pattern_spec(
            "select",
            [
                ("options", options.clone()),
                ("value", Value::string(value)),
                ("onChange", Value::string(event)),
                ("aria-label", Value::string(label)),
            ],
        )
    };
    let before_select = select(&active_before_id, "__vd:before", "Before revision");
    let after_select = select(&active_after_id, "__vd:after", "After revision");
    let badge = |label: String, variant: &str| {
        pattern_spec(
            "badge",
            [
                ("label", Value::string(label)),
                ("variant", Value::string(variant)),
            ],
        )
    };
    let language = get_string(props, "language")
        .filter(|l| !l.is_empty())
        .map(|l| badge(l, "default"));
    let added_badge = badge(format!("+{added}"), "success");
    let removed_badge = badge(format!("-{removed}"), "error");
    let toggle = pattern_spec(
        "button",
        [
            ("label", Value::string("")),
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            (
                "icon",
                Value::string(if side { "align-left" } else { "columns" }),
            ),
            ("action", Value::string("__vd:view")),
            (
                "aria-label",
                Value::string(if side {
                    "Switch to inline view"
                } else {
                    "Switch to side-by-side view"
                }),
            ),
        ],
    );
    let revert = (hunks.is_none() && (on_revert_present(props))).then(|| {
        pattern_spec(
            "button",
            [
                ("label", Value::string("Revert")),
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("rotate-ccw")),
                ("action", Value::string("__vd:revert")),
            ],
        )
    });
    let meta = hunks.is_none()
        && before_rev
            .iter()
            .chain(after_rev.iter())
            .any(|r| r.author.is_some() || r.timestamp.is_some());
    let describe = |rev: Option<&Revision>| {
        let Some(rev) = rev else { return String::new() };
        let mut text = rev.label.clone();
        if let Some(author) = &rev.author {
            text.push_str(&format!(" by {author}"));
        }
        if let Some(timestamp) = &rev.timestamp {
            text.push_str(&format!(" ({timestamp})"));
        }
        text
    };
    let before_meta = typography_spec(
        "caption",
        Some("secondary"),
        "truncate",
        &describe(before_rev),
    );
    let after_meta = typography_spec(
        "caption",
        Some("secondary"),
        "truncate",
        &describe(after_rev),
    );
    let is_empty = hunks.map(|h| h.is_empty()).unwrap_or(false);
    let empty = pattern_spec(
        "typography",
        [
            ("variant", Value::string("body2")),
            ("color", Value::string("muted")),
            ("align", Value::string("center")),
            ("content", Value::string("No changes in this version.")),
        ],
    );

    let header_row = |text: &str| {
        let spec = typography_spec("caption", None, "font-mono text-info", text);
        rsx! { div { class: "{BOX_ATOM_BASE} px-3 py-1 bg-info/10 border-y border-border", {render(&spec)} } }
    };
    let gap_row = |id: usize, count: usize| {
        let spec = pattern_spec(
            "button",
            [
                ("label", Value::string(format!("{count} unchanged lines"))),
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("chevrons-up-down")),
                (
                    "className",
                    Value::string(
                        "w-full justify-start rounded-none bg-muted/40 text-muted-foreground",
                    ),
                ),
                ("action", Value::string("__vd:gap")),
                (
                    "actionPayload",
                    object_value([("id", Value::Number(id as f64))]),
                ),
            ],
        );
        render(&spec)
    };
    let row_elements = |side_of: Option<bool>| -> Vec<Element> {
        shown
            .iter()
            .enumerate()
            .map(|(idx, row)| match row {
                Row::Header(text) => header_row(text),
                Row::Gap { id, count } => gap_row(*id, *count),
                Row::Line(line, context) => match side_of {
                    Some(before_side) => {
                        let hidden = if before_side {
                            line.kind == LineDiffKind::Added
                        } else {
                            line.kind == LineDiffKind::Removed
                        };
                        if hidden {
                            return rsx! {};
                        }
                        let changed = if before_side {
                            line.kind == LineDiffKind::Removed
                        } else {
                            line.kind == LineDiffKind::Added
                        };
                        let bg = match (changed, before_side) {
                            (true, true) => "bg-error/10",
                            (true, false) => "bg-success/10",
                            _ => "",
                        };
                        let color = match (changed, before_side) {
                            (true, true) => "text-error",
                            (true, false) => "text-success",
                            _ => "text-foreground",
                        };
                        let number = line_number_cell(
                            if before_side { line.before_line_number } else { line.after_line_number },
                            "mr-3",
                        );
                        let content = typography_spec(
                            "caption",
                            None,
                            &format!("font-mono flex-1 min-w-0 whitespace-pre {color}"),
                            nonblank(&line.content),
                        );
                        rsx! {
                            div { key: "{idx}", class: "flex flex-row gap-0 items-start justify-start px-3 py-0.5 {bg}",
                                {render(&number)}
                                {render(&content)}
                            }
                        }
                    }
                    None => {
                        let (bg, prefix, text) = match line.kind {
                            LineDiffKind::Added => ("bg-success/10", "+", "text-success"),
                            LineDiffKind::Removed => ("bg-error/10", "-", "text-error"),
                            LineDiffKind::Unchanged if *context => ("", " ", "text-muted-foreground"),
                            LineDiffKind::Unchanged => ("", " ", "text-foreground"),
                        };
                        let shown_content = nonblank(&line.content).to_string();
                        let before_cell = line_number_cell(line.before_line_number, "mr-2");
                        let after_cell = line_number_cell(line.after_line_number, "mr-3");
                        rsx! {
                            div { key: "{idx}", class: "flex flex-row gap-0 items-start justify-start px-4 py-0.5 {bg}",
                                {render(&before_cell)}
                                {render(&after_cell)}
                                div { class: "text-xs font-normal text-foreground font-mono flex-1 min-w-0 whitespace-pre {text}",
                                    span { class: "{BOX_ATOM_BASE} select-none opacity-50 mr-2", "{prefix}" }
                                    "{shown_content}"
                                }
                            }
                        }
                    }
                },
            })
            .collect()
    };
    let before_rows = row_elements(Some(true));
    let after_rows = row_elements(Some(false));
    let inline_rows = row_elements(None);
    let class = format!(
        "{CARD} overflow-hidden {}",
        get_string_or(props, "className", "")
    );

    rsx! {
        div { class: "{class}", style: "height:auto", ..shared_root_attrs(props),
            div { class: "flex flex-col gap-0 items-stretch justify-start",
                div { class: "flex flex-row gap-2 items-center justify-between px-4 py-2 border-b border-border bg-muted/30 flex-wrap",
                    div { class: "flex flex-row gap-2 items-center flex-wrap",
                        {render(&git)}
                        if hunks.is_none() {
                            {render(&compare)}
                            div { class: "{BOX_ATOM_BASE} min-w-0 md:min-w-[160px]", {render(&before_select)} }
                            {render(&to)}
                            div { class: "{BOX_ATOM_BASE} min-w-0 md:min-w-[160px]", {render(&after_select)} }
                        }
                        if let Some(language) = language.as_ref() { {render(language)} }
                        {render(&added_badge)}
                        {render(&removed_badge)}
                    }
                    div { class: "flex flex-row gap-1 items-center",
                        {render(&toggle)}
                        if let Some(revert) = revert.as_ref() { {render(revert)} }
                    }
                }
                if meta {
                    div { class: "flex flex-row gap-2 items-center justify-between px-4 py-1 border-b border-border bg-muted/10",
                        {render(&before_meta)}
                        {render(&after_meta)}
                    }
                }
                div { class: "{BOX_ATOM_BASE} overflow-auto bg-muted/20", style: "max-height:600px", role: "region", tabindex: "0", aria_label: "Changes",
                    if is_empty {
                        div { class: "{BOX_ATOM_BASE} py-8", {render(&empty)} }
                    } else if side {
                        div { class: "{BOX_ATOM_BASE} grid grid-cols-1 md:grid-cols-2",
                            div { class: "{BOX_ATOM_BASE} min-w-0 overflow-x-auto border-b md:border-b-0 md:border-r border-border", div { class: "flex flex-col gap-0 items-stretch justify-start font-mono text-xs", for el in before_rows { {el} } } }
                            div { class: "{BOX_ATOM_BASE} min-w-0 overflow-x-auto", div { class: "flex flex-col gap-0 items-stretch justify-start font-mono text-xs", for el in after_rows { {el} } } }
                        }
                    } else {
                        div { class: "flex flex-col gap-0 items-stretch justify-start font-mono text-xs", for el in inline_rows { {el} } }
                    }
                }
            }
        }
    }
}

fn on_revert_present(props: &Value) -> bool {
    ["onRevert", "revertEvent"]
        .iter()
        .any(|key| get_string(props, key).is_some_and(|name| !name.is_empty()))
}

impl DioxusPatternRenderer for VersionDiffRenderer {
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
        rsx! { Diff { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
