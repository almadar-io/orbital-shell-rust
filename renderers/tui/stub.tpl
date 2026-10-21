//! Auto-generated terminal renderer for the `{{name}}` pattern.
//!
//! This file is a placeholder written by `orb shell-renderers tui`; replace the body by hand.

use crate::PatternRenderer;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Renderer for the `{{name}}` pattern.
pub struct {{Struct}};

impl {{Struct}} {
    const NAME: &str = "{{name}}";
}

impl PatternRenderer for {{Struct}} {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        // 🔧 {{name}} placeholder
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        frame.render_widget(
            Paragraph::new(Self::NAME).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }
}
