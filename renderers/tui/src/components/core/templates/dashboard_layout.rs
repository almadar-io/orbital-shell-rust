//! Terminal body for the `dashboard-layout` pattern, following the Dioxus port.

use crate::{PatternRenderer, TuiChildTarget};
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::child_patterns;
use orbital_core::{Value, ValueMap};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const SIDEBAR_WIDTH: u16 = 26;
const TOP_BAR_HEIGHT: u16 = 3;
const HEADER_ACTIONS_WIDTH: u16 = 28;
const THEME_TOGGLE_WIDTH: u16 = 6;

/// Renderer for the `dashboard-layout` pattern.
pub struct DashboardLayoutRenderer;

impl DashboardLayoutRenderer {
    const NAME: &str = "dashboard-layout";
}

fn slot<'a>(props: &'a Value, key: &str) -> Option<&'a Value> {
    props.get(key).filter(|value| match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        _ => true,
    })
}

fn draw_node(frame: &mut Frame, area: Rect, ctx: &mut RenderContext<'_>, node: &Value) {
    let mut target = TuiChildTarget { frame, area };
    ctx.render_one(&mut target as *mut TuiChildTarget<'_, '_> as *mut (), node);
}

fn active_href(items: &[Value], current: &str) -> Option<String> {
    if current.is_empty() {
        return None;
    }
    let mut best: Option<String> = None;
    for item in items {
        let mut hrefs = vec![get_string_or(item, "href", "")];
        for child in get_array(item, "children") {
            hrefs.push(get_string_or(&child, "href", ""));
        }
        for href in hrefs {
            if current == href {
                return Some(href);
            }
            if current.starts_with(&format!("{href}/"))
                && best.as_ref().is_none_or(|known| href.len() > known.len())
            {
                best = Some(href);
            }
        }
    }
    best
}

fn theme_toggle_spec() -> Value {
    let mut spec = ValueMap::new();
    spec.insert("type".into(), Value::string("theme-toggle"));
    spec.insert("size".into(), Value::string("md"));
    Value::Object(spec.into())
}

impl PatternRenderer for DashboardLayoutRenderer {
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
        let app_name = get_string_or(props, "appName", "App");
        let nav_items = get_array(props, "navItems");
        let current_path = get_string_or(props, "currentPath", "");
        let active = active_href(&nav_items, &current_path);
        let show_theme_toggle = get_bool_or(props, "showThemeToggle", true);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
            .split(area);

        let sidebar = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(app_name);
        let sidebar_inner = sidebar.inner(columns[0]);
        frame.render_widget(sidebar, columns[0]);

        let sidebar_content = slot(props, "sidebarContent");
        let sidebar_footer = slot(props, "sidebarFooter");
        let mut sidebar_rows = vec![
            Constraint::Length(nav_items.len().min(u16::MAX as usize) as u16),
            Constraint::Min(0),
        ];
        if sidebar_footer.is_some() {
            sidebar_rows.push(Constraint::Length(3));
        }
        let sidebar_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(sidebar_rows)
            .split(sidebar_inner);
        let nav_lines: Vec<Line> = nav_items
            .iter()
            .map(|item| {
                let label = get_string_or(item, "label", "");
                let href = get_string_or(item, "href", "");
                let style = if active.as_deref() == Some(href.as_str()) {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::styled(label, style)
            })
            .collect();
        frame.render_widget(Paragraph::new(nav_lines), sidebar_areas[0]);
        if let Some(node) = sidebar_content {
            draw_node(frame, sidebar_areas[1], ctx, node);
        }
        if let Some(node) = sidebar_footer {
            draw_node(frame, sidebar_areas[2], ctx, node);
        }

        let column = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(TOP_BAR_HEIGHT), Constraint::Min(0)])
            .split(columns[1]);

        let header_actions = slot(props, "headerActions");
        let mut bar_cells = vec![Constraint::Min(0)];
        if header_actions.is_some() {
            bar_cells.push(Constraint::Length(HEADER_ACTIONS_WIDTH));
        }
        if show_theme_toggle {
            bar_cells.push(Constraint::Length(THEME_TOGGLE_WIDTH));
        }
        let bar = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(bar_cells)
            .split(column[0]);
        let mut cell = 1;
        if let Some(node) = header_actions {
            draw_node(frame, bar[cell], ctx, node);
            cell += 1;
        }
        if show_theme_toggle {
            draw_node(frame, bar[cell], ctx, &theme_toggle_spec());
        }

        let children = child_patterns(props, ctx.trait_content);
        if children.is_empty() {
            return;
        }
        let main_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(3); children.len()])
            .split(column[1]);
        for ((child, child_source), child_area) in children.iter().zip(main_areas.iter()) {
            let previous = ctx.source_trait.clone();
            if child_source.is_some() {
                ctx.source_trait = child_source.clone();
            }
            draw_node(frame, *child_area, ctx, child);
            ctx.source_trait = previous;
        }
    }
}
