//! Terminal renderer for the `code-block` pattern.
//!
//! Hand-authored: the viewer path (no `cursor` prop) is a bordered paragraph; buffer mode (`cursor` is an
//! object) paints the program-owned editor surface and answers raw key and pointer input as events. The
//! program owns the text, cursor and every key's meaning; this surface only paints and reports.

use crate::PatternRenderer;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use orbital_core::client::helpers::*;
use orbital_core::Value;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::Frame;

/// Renderer for the `code-block` pattern.
pub struct CodeBlockRenderer;

impl CodeBlockRenderer {
    const NAME: &str = "code-block";
}

impl PatternRenderer for CodeBlockRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        use ratatui::widgets::Paragraph;
        if is_buffer_mode(props) {
            let caret = paint_buffer(frame.buffer_mut(), area, props);
            if let Some(position) = caret.filter(|_| caret_kind(props) == Caret::Bar) {
                frame.set_cursor_position(position);
            }
            return;
        }
        let code = get_string_or(props, "code", get_string_or(props, "content", "").as_str());
        frame.render_widget(
            Paragraph::new(code).block(Block::default().borders(Borders::ALL).title("code")),
            area,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Caret {
    Block,
    Bar,
    Underline,
}

type Pos = (usize, usize);

struct Range {
    from: Pos,
    to: Pos,
}

struct Selection {
    range: Range,
    linewise: bool,
}

struct Decoration {
    range: Range,
    tone: String,
}

struct Popover {
    anchor: Pos,
    lines: Vec<(String, bool)>,
}

struct Model {
    lines: Vec<Vec<char>>,
    cursor: Pos,
    caret: Caret,
    selection: Option<Selection>,
    decorations: Vec<Decoration>,
    popover: Option<Popover>,
    line_numbers: bool,
    relative: bool,
}

struct Geometry {
    gutter: u16,
    top: usize,
    left: usize,
    text: Rect,
}

pub fn is_buffer_mode(props: &Value) -> bool {
    props
        .get("cursor")
        .map(|c| c.as_object().is_some())
        .unwrap_or(false)
}

fn caret_kind(props: &Value) -> Caret {
    match get_string(props, "caret").as_deref() {
        Some("bar") => Caret::Bar,
        Some("underline") => Caret::Underline,
        _ => Caret::Block,
    }
}

fn index(v: &serde_json::Value, key: &str) -> Option<usize> {
    v.get(key)?.as_f64().map(|n| n.max(0.0) as usize)
}

fn range_of(v: &serde_json::Value) -> Option<Range> {
    Some(Range {
        from: (index(v, "fromRow")?, index(v, "fromCol")?),
        to: (index(v, "toRow")?, index(v, "toCol")?),
    })
}

fn model(props: &Value) -> Model {
    let json = to_json(props);
    let code = json
        .get("code")
        .or_else(|| json.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let cursor = json
        .get("cursor")
        .map(|c| (index(c, "row").unwrap_or(0), index(c, "col").unwrap_or(0)))
        .unwrap_or((0, 0));
    let selection = json
        .get("selection")
        .filter(|s| s.is_object())
        .and_then(|s| {
            let linewise = s.get("linewise").and_then(|l| l.as_bool()).unwrap_or(false);
            let a = (index(s, "fromRow")?, index(s, "fromCol")?);
            let b = (index(s, "toRow")?, index(s, "toCol")?);
            Some(Selection {
                range: Range {
                    from: a.min(b),
                    to: a.max(b),
                },
                linewise,
            })
        });
    let decorations = json
        .get("decorations")
        .and_then(|d| d.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|d| {
                    Some(Decoration {
                        range: range_of(d)?,
                        tone: d.get("tone")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let popover = json.get("popover").filter(|p| p.is_object()).and_then(|p| {
        let range = range_of(p)?;
        let mut lines = vec![(
            p.get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
            true,
        )];
        for section in p
            .get("sections")
            .and_then(|s| s.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(heading) = section.get("heading").and_then(|h| h.as_str()) {
                lines.push((heading.to_string(), true));
            }
            for line in section
                .get("lines")
                .and_then(|l| l.as_array())
                .into_iter()
                .flatten()
            {
                lines.push((line.as_str().unwrap_or("").to_string(), false));
            }
        }
        if p.get("busy").and_then(|b| b.as_bool()).unwrap_or(false) {
            lines.push(("…".to_string(), false));
        }
        Some(Popover {
            anchor: range.from,
            lines,
        })
    });
    Model {
        lines: code.split('\n').map(|l| l.chars().collect()).collect(),
        cursor,
        caret: caret_kind(props),
        selection,
        decorations,
        popover,
        line_numbers: get_bool_or(props, "lineNumbers", false),
        relative: get_bool_or(props, "relativeLineNumbers", false),
    }
}

fn geometry(model: &Model, area: Rect) -> Geometry {
    let gutter = if model.line_numbers {
        (model.lines.len().to_string().len() as u16 + 1).min(area.width)
    } else {
        0
    };
    let text = Rect {
        x: area.x + gutter,
        y: area.y,
        width: area.width - gutter,
        height: area.height,
    };
    let top = (model.cursor.0 + 1).saturating_sub(text.height as usize);
    let left = (model.cursor.1 + 1).saturating_sub(text.width as usize);
    Geometry {
        gutter,
        top,
        left,
        text,
    }
}

fn tone_style(tone: &str) -> Style {
    let fg = |c: Color| Style::default().fg(c);
    let bg = |b: Color, f: Color| Style::default().bg(b).fg(f);
    match tone {
        "keyword" => fg(Color::Magenta),
        "string" => fg(Color::Green),
        "comment" => fg(Color::DarkGray),
        "number" => fg(Color::Yellow),
        "type" => fg(Color::Cyan),
        "function" => fg(Color::Blue),
        "operator" => fg(Color::Red),
        "punctuation" => fg(Color::Gray),
        "variable" => fg(Color::White),
        "search" => bg(Color::Yellow, Color::Black),
        "review" => bg(Color::Blue, Color::White),
        "review-recent" => bg(Color::LightBlue, Color::Black),
        "error" => bg(Color::Red, Color::White),
        "warning" => fg(Color::Yellow).add_modifier(Modifier::UNDERLINED),
        "info" => bg(Color::Cyan, Color::Black),
        _ => Style::default(),
    }
}

fn within(range: &Range, at: Pos) -> bool {
    range.from <= at && at < range.to
}

fn cell_style(model: &Model, at: Pos) -> Style {
    let mut style = Style::default();
    for decoration in &model.decorations {
        if within(&decoration.range, at) {
            style = style.patch(tone_style(&decoration.tone));
        }
    }
    if let Some(selection) = &model.selection {
        let selected = if selection.linewise {
            selection.range.from.0 <= at.0 && at.0 <= selection.range.to.0
        } else {
            selection.range.from <= at && at <= selection.range.to
        };
        if selected {
            style = style.bg(Color::DarkGray);
        }
    }
    if at == model.cursor {
        match model.caret {
            Caret::Block => style = style.add_modifier(Modifier::REVERSED),
            Caret::Underline => style = style.add_modifier(Modifier::UNDERLINED),
            Caret::Bar => {}
        }
    }
    style
}

fn gutter_label(model: &Model, row: usize) -> usize {
    if model.relative && row != model.cursor.0 {
        row.abs_diff(model.cursor.0)
    } else {
        row + 1
    }
}

/// Paints buffer mode into `area`; returns the screen cell of the caret when it is visible.
pub fn paint_buffer(buf: &mut Buffer, area: Rect, props: &Value) -> Option<Position> {
    let model = model(props);
    let geo = geometry(&model, area);
    let mut caret = None;
    for screen_row in 0..area.height {
        let row = geo.top + screen_row as usize;
        let Some(line) = model.lines.get(row) else {
            break;
        };
        let y = area.y + screen_row;
        if geo.gutter > 0 {
            let label = format!(
                "{:>width$} ",
                gutter_label(&model, row),
                width = geo.gutter as usize - 1
            );
            let mut style = Style::default().fg(Color::DarkGray);
            if row == model.cursor.0 {
                style = style.fg(Color::White).add_modifier(Modifier::BOLD);
            }
            buf.set_stringn(area.x, y, label, geo.gutter as usize, style);
        }
        let linewise = model
            .selection
            .as_ref()
            .is_some_and(|s| s.linewise && s.range.from.0 <= row && row <= s.range.to.0);
        for screen_col in 0..geo.text.width {
            let col = geo.left + screen_col as usize;
            let x = geo.text.x + screen_col;
            let ch = line.get(col).copied();
            let past_end = col > line.len();
            if past_end && !linewise {
                break;
            }
            let at = (row, col);
            let style = if past_end {
                Style::default().bg(Color::DarkGray)
            } else {
                cell_style(&model, at)
            };
            let cell = &mut buf[(x, y)];
            cell.set_char(match ch {
                Some('\t') | None => ' ',
                Some(c) => c,
            });
            cell.set_style(style);
            if at == model.cursor {
                caret = Some(Position::new(x, y));
            }
        }
    }
    if let Some(popover) = &model.popover {
        paint_popover(buf, area, &geo, popover);
    }
    caret
}

fn paint_popover(buf: &mut Buffer, area: Rect, geo: &Geometry, popover: &Popover) {
    let width = popover
        .lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0) as u16
        + 2;
    let height = popover.lines.len() as u16 + 2;
    let width = width.min(area.width);
    let height = height.min(area.height);
    let anchor_row = popover.anchor.0.saturating_sub(geo.top) as u16;
    let anchor_col = popover.anchor.1.saturating_sub(geo.left) as u16;
    let x = (geo.text.x + anchor_col).min(area.x + area.width - width);
    let below = anchor_row + 1;
    let y = if below + height <= area.height {
        area.y + below
    } else if anchor_row >= height {
        area.y + anchor_row - height
    } else {
        area.y + area.height - height
    };
    let rect = Rect {
        x,
        y,
        width,
        height,
    };
    let surface = Style::default().bg(Color::Black).fg(Color::White);
    Clear.render(rect, buf);
    let frame = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(surface);
    let inner = frame.inner(rect);
    frame.render(rect, buf);
    for (i, (text, strong)) in popover.lines.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let style = if *strong {
            surface.add_modifier(Modifier::BOLD)
        } else {
            surface
        };
        buf.set_stringn(
            inner.x,
            inner.y + i as u16,
            text,
            inner.width as usize,
            style,
        );
    }
}

/// The text cell under screen cell `(x, y)` of a buffer-mode block painted into `area`; a press past a line's end
/// gives the line length and a press below the last line gives the last row.
pub fn pointer_cell(props: &Value, area: Rect, x: u16, y: u16) -> Option<Pos> {
    if x < area.x || y < area.y || x >= area.x + area.width || y >= area.y + area.height {
        return None;
    }
    let model = model(props);
    let geo = geometry(&model, area);
    let row = (geo.top + (y - area.y) as usize).min(model.lines.len() - 1);
    let col = geo.left + x.saturating_sub(geo.text.x) as usize;
    Some((row, col.min(model.lines[row].len())))
}

/// The `onKey` payload of a terminal key event; `None` for a release or a key the web spells with no name here.
pub fn key_payload(editor_id: &str, event: &KeyEvent) -> Option<Value> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let mut shift = event.modifiers.contains(KeyModifiers::SHIFT);
    let (key, code) = match event.code {
        KeyCode::Char(' ') => (" ".to_string(), "Space".to_string()),
        KeyCode::Char(c) => {
            let code = if c.is_ascii_alphabetic() {
                format!("Key{}", c.to_ascii_uppercase())
            } else if c.is_ascii_digit() {
                format!("Digit{c}")
            } else {
                String::new()
            };
            (c.to_string(), code)
        }
        KeyCode::Esc => ("Escape".into(), "Escape".into()),
        KeyCode::Enter => ("Enter".into(), "Enter".into()),
        KeyCode::Backspace => ("Backspace".into(), "Backspace".into()),
        KeyCode::Tab => ("Tab".into(), "Tab".into()),
        KeyCode::BackTab => {
            shift = true;
            ("Tab".into(), "Tab".into())
        }
        KeyCode::Left => ("ArrowLeft".into(), "ArrowLeft".into()),
        KeyCode::Right => ("ArrowRight".into(), "ArrowRight".into()),
        KeyCode::Up => ("ArrowUp".into(), "ArrowUp".into()),
        KeyCode::Down => ("ArrowDown".into(), "ArrowDown".into()),
        KeyCode::Home => ("Home".into(), "Home".into()),
        KeyCode::End => ("End".into(), "End".into()),
        KeyCode::PageUp => ("PageUp".into(), "PageUp".into()),
        KeyCode::PageDown => ("PageDown".into(), "PageDown".into()),
        KeyCode::Delete => ("Delete".into(), "Delete".into()),
        KeyCode::Insert => ("Insert".into(), "Insert".into()),
        KeyCode::F(n) => (format!("F{n}"), format!("F{n}")),
        _ => return None,
    };
    Some(from_json(serde_json::json!({
        "editorId": editor_id,
        "key": key,
        "code": code,
        "ctrl": event.modifiers.contains(KeyModifiers::CONTROL),
        "alt": event.modifiers.contains(KeyModifiers::ALT),
        "shift": shift,
        "meta": event.modifiers.contains(KeyModifiers::SUPER),
        "repeat": event.kind == KeyEventKind::Repeat,
    })))
}

/// Queues the block's `onKey` event for a key press while it is focused; false when it declares no `onKey`.
pub fn emit_key(props: &Value, ctx: &mut RenderContext<'_>, event: &KeyEvent) -> bool {
    let Some(name) = get_string(props, "onKey") else {
        return false;
    };
    let Some(payload) = key_payload(&get_string_or(props, "editorId", ""), event) else {
        return false;
    };
    ctx.queue_event(name, payload);
    true
}

/// Queues the block's `onPointerDown` or `onHover` event (named by `prop`) for a pointer at screen cell `(x, y)`.
pub fn emit_pointer(
    props: &Value,
    ctx: &mut RenderContext<'_>,
    area: Rect,
    prop: &str,
    x: u16,
    y: u16,
) -> bool {
    let Some(name) = get_string(props, prop) else {
        return false;
    };
    let Some((row, col)) = pointer_cell(props, area, x, y) else {
        return false;
    };
    ctx.queue_event(
        name,
        from_json(serde_json::json!({
            "editorId": get_string_or(props, "editorId", ""),
            "row": row,
            "col": col,
        })),
    );
    true
}
