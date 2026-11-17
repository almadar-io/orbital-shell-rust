//! Terminal renderer for the `annotated-media` pattern: a terminal cannot click on media, so it lists the media and its numbered pins, and a pin is selected by number.

use crate::{PatternRenderer, TuiChildTarget};
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

/// Renderer for the `annotated-media` pattern.
pub struct AnnotatedMediaRenderer;

impl AnnotatedMediaRenderer {
    const NAME: &str = "annotated-media";
}

fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).floor() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

fn pin_where(pin: &Value, video: bool) -> Option<String> {
    let time = if video { get_number(pin, "time") } else { None };
    match (get_number(pin, "x"), get_number(pin, "y"), time) {
        (Some(x), Some(y), _) => Some(format!("({}%, {}%)", x, y)),
        (_, _, Some(time)) => Some(format!("@ {}", clock(time))),
        _ => None,
    }
}

/// Queues the `pinSelectEvent` for the pin numbered `number` (1-based); false when the pattern declares none or no such pin exists.
pub fn emit_pin_select(props: &Value, ctx: &mut RenderContext<'_>, number: usize) -> bool {
    let Some(name) = get_string(props, "pinSelectEvent") else {
        return false;
    };
    let Some(pin) = number
        .checked_sub(1)
        .and_then(|index| get_array(props, "pins").into_iter().nth(index))
    else {
        return false;
    };
    let Some(id) = get_string(&pin, "id") else {
        return false;
    };
    ctx.queue_event(name, from_json(serde_json::json!({ "id": id })));
    true
}

impl PatternRenderer for AnnotatedMediaRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        ctx: &mut RenderContext<'_>,
    ) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let media_type = get_string_or(props, "mediaType", "image");
        let video = media_type == "video";
        let content = media_type == "content";
        let active = get_string(props, "activePinId");
        let title = get_string_or(props, "title", "");
        let mut lines = Vec::new();
        if !title.is_empty() {
            lines.push(Line::from(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        if !content {
            lines.push(Line::from(format!(
                "{}: {}",
                if video { "video" } else { "image" },
                get_string_or(props, "src", "")
            )));
        }
        for (index, pin) in get_array(props, "pins").iter().enumerate() {
            let label = get_string_or(pin, "label", "");
            let mut text = format!("{}.", index + 1);
            if !label.is_empty() {
                text.push(' ');
                text.push_str(&label);
            }
            if let Some(position) = pin_where(pin, video) {
                text.push(' ');
                text.push_str(&position);
            }
            let is_active = active.is_some() && get_string(pin, "id") == active;
            let resolved = get_bool_or(pin, "resolved", false);
            let style = if is_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if resolved {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(text, style)));
        }

        let mut pin_area = area;
        if content {
            let height = (lines.len() as u16).saturating_add(2).min(area.height);
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(height)])
                .split(area);
            let children = child_patterns(props, ctx.trait_content);
            if !children.is_empty() {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![Constraint::Min(1); children.len()])
                    .split(parts[0]);
                for ((child, child_source), child_area) in children.iter().zip(rows.iter()) {
                    let previous = ctx.source_trait.clone();
                    if child_source.is_some() {
                        ctx.source_trait = child_source.clone();
                    }
                    let mut target = TuiChildTarget {
                        frame: &mut *frame,
                        area: *child_area,
                    };
                    ctx.render_one(&mut target as *mut TuiChildTarget<'_, '_> as *mut (), child);
                    ctx.source_trait = previous;
                }
            }
            pin_area = parts[1];
        }

        frame.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            pin_area,
        );
    }
}
