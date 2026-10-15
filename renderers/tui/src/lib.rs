//! Terminal UI Rendering
//!
//! Standalone terminal application using ratatui + crossterm.

// Generated renderers: unused imports/params vary per pattern body.
#[allow(dead_code, unused_variables, unused_imports)]
pub mod components;

use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::client_core::ClientCore;
use orbital_core::client::client_kernel::ClientKernel;
use orbital_core::client::http::SyncServerBridge;
use orbital_core::client::helpers::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use orbital_core::{OrbitalSchema, Value};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::collections::HashMap;
use std::io::{self, Stdout};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info};

/// Terminal rendering errors
#[derive(Debug, Error)]
pub enum TuiError {
    #[error("Failed to parse schema: {0}")]
    Schema(String),
    #[error("Failed to run application: {0}")]
    Runtime(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Trait for rendering a named UI pattern into a terminal area.
pub trait PatternRenderer: Send + Sync {
    /// The pattern this renderer handles (e.g. "page-header").
    fn pattern_name(&self) -> &str;

    /// Render the pattern into the given terminal area.
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        entity: Option<&Value>,
        ctx: &mut RenderContext<'_>,
    );
}

/// Registry mapping pattern names to their terminal renderers.
pub struct PatternRegistry {
    renderers: HashMap<String, Box<dyn PatternRenderer>>,
    fallback: Box<dyn PatternRenderer>,
}

impl PatternRegistry {
    /// Create a registry pre-populated with the default terminal renderers.
    pub fn new() -> Self {
        let fallback = Box::new(UnsupportedPatternRenderer);
        let mut registry = Self {
            renderers: HashMap::new(),
            fallback,
        };
        registry.register_defaults();
        registry
    }

    fn register_defaults(&mut self) {
        crate::components::core::register::register_terminal(self);

        self.register(Box::new(PageHeaderRenderer));
        self.register(Box::new(EntityDetailRenderer));
        self.register(Box::new(FormSectionRenderer));
        self.register(Box::new(StatCardRenderer));
        self.register(Box::new(MessageRenderer));
        self.register(Box::new(LoadingStateRenderer));
        self.register(Box::new(EmptyStateRenderer));
        self.register(Box::new(ErrorStateRenderer));
        self.register(Box::new(ButtonRenderer));
    }

    /// Register a custom renderer, replacing any existing renderer for the same pattern.
    pub fn register(&mut self, renderer: Box<dyn PatternRenderer>) {
        let name = renderer.pattern_name().to_string();
        self.renderers.insert(name, renderer);
    }

    /// The patterns this target implements, sorted — its declared coverage list.
    pub fn coverage(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.renderers.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Whether `name` has a renderer here.
    pub fn supports(&self, name: &str) -> bool {
        self.renderers.contains_key(name)
    }

    /// Look up a renderer by pattern name, else the unsupported-pattern answer.
    pub fn get_renderer(&self, name: &str) -> &dyn PatternRenderer {
        self.renderers
            .get(name)
            .map(|r| r.as_ref())
            .unwrap_or_else(|| self.fallback.as_ref())
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Request log entry for debugging and tracking
#[derive(Debug, Clone)]
pub struct RequestLog {
    pub event: String,
    pub timestamp: i64,
    pub success: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// The transport for a terminal app: a server at `server_url`, else the
/// in-process kernel (offline, seeded like every headless client).
fn bridge(schema: &OrbitalSchema, server_url: Option<&str>) -> anyhow::Result<Arc<dyn ServerBridge>> {
    Ok(match server_url {
        Some(url) => Arc::new(SyncServerBridge::new(url)),
        None => {
            let kernel = KernelBridge::new(schema.clone()).map_err(|e| anyhow::anyhow!("kernel bridge: {e}"))?;
            kernel.seed_sample(schema, 3);
            Arc::new(kernel)
        }
    })
}

/// Run the terminal UI application with the default renderer set.
pub fn run(schema_path: &Path, server_url: Option<&str>) -> anyhow::Result<()> {
    run_with_renderers(schema_path, server_url, Vec::new())
}

/// Run the terminal UI application with additional custom renderers.
pub fn run_with_renderers(
    schema_path: &Path,
    server_url: Option<&str>,
    renderers: Vec<Box<dyn PatternRenderer>>,
) -> anyhow::Result<()> {
    let schema = orbital_core::schema::parse_schema_file(schema_path)?;
    let bridge = bridge(&schema, server_url)?;
    let mut app = TuiApp::new(schema, Some(bridge), renderers)?;

    setup_terminal()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = run_app(&mut terminal, &mut app);
    restore_terminal()?;
    result
}

fn setup_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut TuiApp) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| app.draw(frame))?;

        // One round of the events queued during rendering, per frame.
        app.settle_round();

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key.code) {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// A terminal host over the shared client circuit ([`ClientCore`]).
pub struct TuiApp {
    core: ClientCore,
    request_history: Vec<RequestLog>,
    pattern_registry: PatternRegistry,
}

impl TuiApp {
    /// Build the app over `bridge` (none: offline) and mount the first page.
    pub fn new(
        schema: OrbitalSchema,
        bridge: Option<Arc<dyn ServerBridge>>,
        renderers: Vec<Box<dyn PatternRenderer>>,
    ) -> anyhow::Result<Self> {
        info!(schema = %schema.name, "Creating TUI app");
        let core = match bridge {
            Some(bridge) => {
                let kernel = ClientKernel::new(schema.clone(), Some(bridge))
                    .map_err(|e| anyhow::anyhow!("client kernel: {e}"))?;
                ClientCore::with_client_kernel(schema, Arc::new(kernel))
            }
            None => ClientCore::new(schema, None),
        };
        Ok(Self::with_core(core, renderers))
    }

    /// Build the app over a circuit the caller already wired (its client kernel, seeded rows, viewer) and mount
    /// the first page.
    pub fn with_core(core: ClientCore, renderers: Vec<Box<dyn PatternRenderer>>) -> Self {
        let mut pattern_registry = PatternRegistry::new();
        for renderer in renderers {
            pattern_registry.register(renderer);
        }
        let mut app = Self { core, request_history: Vec::new(), pattern_registry };
        app.mount();
        app
    }

    /// The shared circuit this app draws from.
    pub fn core(&self) -> &ClientCore {
        &self.core
    }

    /// The shared circuit, to drive headlessly.
    pub fn core_mut(&mut self) -> &mut ClientCore {
        &mut self.core
    }

    /// The renderer registry (its `supports`/`coverage` are this target's pattern list).
    pub fn registry(&self) -> &PatternRegistry {
        &self.pattern_registry
    }

    fn mount(&mut self) {
        if !self.core.is_online() {
            self.core.handle_offline_event("INIT");
            self.core.status_message = Some(("Offline mode: Connect to server for full functionality".to_string(), true));
            return;
        }
        self.timed("INIT", |core| {
            core.mount_page_blocking();
        });
    }

    /// Run `drive` against the core, logging it like a request.
    fn timed(&mut self, event: &str, drive: impl FnOnce(&mut ClientCore)) {
        let start = web_time::Instant::now();
        let timestamp = chrono::Utc::now().timestamp_millis();
        self.core.status_message = None;
        drive(&mut self.core);
        let error = self
            .core
            .status_message
            .as_ref()
            .filter(|(_, is_error)| *is_error)
            .map(|(message, _)| message.clone());
        if let Some(e) = &error {
            error!(event = %event, error = %e, "Event failed");
        }
        self.request_history.push(RequestLog {
            event: event.to_string(),
            timestamp,
            success: error.is_none(),
            duration_ms: start.elapsed().as_millis() as u64,
            error,
        });
        if self.request_history.len() > 50 {
            self.request_history.remove(0);
        }
    }

    /// Dispatch `event` as the active trait.
    pub fn send_event(&mut self, event: &str, payload: Value) {
        if !self.core.is_online() {
            self.core.handle_offline_event(event);
            self.core.status_message = Some((format!("Cannot execute '{}' in offline mode", event), true));
            return;
        }
        debug!(trait_name = %self.core.current_trait_name, event = %event, "Sending event through client core");
        self.timed(event, |core| core.drive_blocking(event, None, payload));
    }

    /// Drive one round of queued follow-up events (each as the trait that emitted it).
    pub fn settle_round(&mut self) {
        for (event, origin, payload) in std::mem::take(&mut self.core.pending_events) {
            self.timed(&event, |core| core.drive_blocking(&event, origin.as_deref(), payload));
        }
    }

    /// Handle keyboard input. Returns true when the app should quit.
    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return true,
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = c.to_digit(10).unwrap_or(0) as usize;
                if digit > 0 && digit <= self.core.schema.orbitals.len() {
                    self.switch_orbital(digit - 1);
                }
            }
            KeyCode::Esc => {
                self.core.form_data.clear();
                self.core.editing_entity = None;
                self.send_event("CANCEL", Value::Null);
            }
            _ => {}
        }
        false
    }

    fn switch_orbital(&mut self, idx: usize) {
        self.core.set_orbital(idx);
        self.core.form_data.clear();
        self.mount();
    }

    /// Draw one frame.
    pub fn draw(&mut self, frame: &mut Frame) {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Percentage(80)])
            .split(frame.area());

        let content_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(main_layout[1]);

        self.render_sidebar(frame, main_layout[0]);
        self.render_main_content(frame, content_layout[0]);
        self.render_status_bar(frame, content_layout[1]);
    }

    fn render_sidebar(&self, frame: &mut Frame, area: Rect) {
        let current = self.core.current_orbital_idx;
        let orbital_items: Vec<ListItem> = self
            .core
            .schema
            .orbitals
            .iter()
            .enumerate()
            .map(|(idx, orbital)| {
                let prefix = if idx == current { "> " } else { "  " };
                let style = if idx == current {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(vec![
                    Span::raw(prefix),
                    Span::raw(format!("{}.", idx + 1)),
                    Span::raw(" "),
                    Span::raw(orbital.name.as_str()),
                ]))
                .style(style)
            })
            .collect();

        let connection = if self.core.is_online() {
            Span::styled("Connected", Style::default().fg(Color::Green))
        } else {
            Span::styled("Offline", Style::default().fg(Color::Red))
        };
        let footer = Line::from(vec![
            Span::raw("State: "),
            Span::raw(self.core.current_state.as_str()),
            Span::raw(" | "),
            connection,
        ]);

        let list = List::new(orbital_items)
            .block(Block::default().title("Orbitals").borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        frame.render_widget(list, area);
        frame.render_widget(Paragraph::new(footer), inner_rect(area, 0, area.height.saturating_sub(2), area.width, 1));
    }

    fn render_main_content(&mut self, frame: &mut Frame, area: Rect) {
        let main_contents = self.core.slot_contents("main");
        if main_contents.is_empty() {
            let empty = Paragraph::new("No content to display")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::NONE));
            frame.render_widget(empty, area);
            return;
        }

        let orbital_name = self.core.orbital_name();
        let core = &mut self.core;
        let trait_content: orbital_core::ValueMap = core
            .trait_content
            .iter()
            .map(|(name, content)| (name.clone(), resolve_row_set_markers(content, &|reference| core.resolve_rows(reference))))
            .collect();
        let areas: Vec<Rect> = Layout::default()
            .direction(Direction::Vertical)
            .constraints(std::iter::repeat_n(Constraint::Min(3), main_contents.len()).collect::<Vec<_>>())
            .split(area)
            .to_vec();

        for (content, area) in main_contents.iter().zip(areas) {
            let renderer = self.pattern_registry.get_renderer(&content.pattern);
            let painted = resolve_row_set_markers(&content.props, &|reference| core.resolve_rows(reference));
            let mut ctx = RenderContext {
                trait_name: core.current_trait_name.as_str(),
                orbital_name: orbital_name.as_str(),
                current_state: core.current_state.as_str(),
                entities: &core.entities,
                form_data: &mut core.form_data,
                editing_entity: &core.editing_entity,
                pending_events: &mut core.pending_events,
                child_render: None,
                trait_content: &trait_content,
                source_trait: content.source_trait.clone(),
                canvas_theme: None,
            };
            renderer.render(frame, area, &painted, content.entity.as_ref(), &mut ctx);
        }
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let mut spans = vec![Span::raw("q:quit "), Span::raw("1-9:orbitals "), Span::raw("esc:cancel ")];
        if let Some((message, is_error)) = &self.core.status_message {
            spans.push(Span::raw(" | "));
            let style = if *is_error { Style::default().fg(Color::Red) } else { Style::default().fg(Color::Green) };
            spans.push(Span::styled(message.clone(), style));
        }
        let total = self.request_history.len();
        let failed = self.request_history.iter().filter(|r| !r.success).count();
        let avg = if total > 0 {
            self.request_history.iter().map(|r| r.duration_ms).sum::<u64>() / total as u64
        } else {
            0
        };
        spans.push(Span::raw(format!(" | req:{} fail:{} avg:{}ms", total, failed, avg)));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// The frame as text, `width` × `height` cells: the terminal renderer's screenshot.
    pub fn render_to_text(&mut self, width: u16, height: u16) -> anyhow::Result<String> {
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))?;
        terminal.draw(|frame| self.draw(frame))?;
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        Ok(out)
    }
}

fn inner_rect(area: Rect, x: u16, y: u16, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + x,
        y: area.y + y,
        width: width.min(area.width.saturating_sub(x)),
        height: height.min(area.height.saturating_sub(y)),
    }
}

// =============================================================================
// Default pattern renderers
// =============================================================================

pub struct PageHeaderRenderer;

impl PatternRenderer for PageHeaderRenderer {
    fn pattern_name(&self) -> &str {
        "page-header"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let title = get_string_or(props, "title", "Untitled");
        let subtitle = get_string(props, "subtitle");
        let actions = get_actions(props, "actions");

        let mut text_lines = vec![Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))];

        if let Some(sub) = subtitle {
            text_lines.push(Line::from(Span::styled(
                sub,
                Style::default().fg(Color::Gray),
            )));
        }

        if !actions.is_empty() {
            let action_spans: Vec<Span> = actions
                .iter()
                .map(|a| {
                    Span::styled(
                        format!("[{}] ", a.label),
                        Style::default().fg(Color::Yellow),
                    )
                })
                .collect();
            text_lines.push(Line::from(action_spans));
        }

        let header =
            Paragraph::new(Text::from(text_lines)).block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(header, area);
    }
}

pub struct EntityDetailRenderer;

impl PatternRenderer for EntityDetailRenderer {
    fn pattern_name(&self) -> &str {
        "entity-detail"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        ctx: &mut RenderContext<'_>,
    ) {
        let entity = get_string_or(props, "entity", "Unknown");
        let entity_id = get_string_or(props, "entityId", "");
        let field_names = get_string_array(props, "fields");
        let items = ctx.entity_data(&entity).unwrap_or(&[]);

        let item = items.iter().find(|i| {
            get_entity_field(i, "id")
                .and_then(|v| v.as_str().map(|s| s == entity_id))
                .unwrap_or(false)
        });

        if let Some(data) = item {
            let lines: Vec<Line> = field_names
                .iter()
                .filter_map(|field| {
                    get_entity_field(data, field).map(|value| {
                        Line::from(vec![
                            Span::styled(
                                format!("{}: ", humanize_field_name(field)),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(format_value(&value)),
                        ])
                    })
                })
                .collect();

            let detail = Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .title(format!("{} Detail", entity))
                    .borders(Borders::ALL),
            );
            frame.render_widget(detail, area);
        } else {
            frame.render_widget(
                Paragraph::new(format!("Entity not found: {}", entity_id)),
                area,
            );
        }
    }
}

pub struct FormSectionRenderer;

impl PatternRenderer for FormSectionRenderer {
    fn pattern_name(&self) -> &str {
        "form-section"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        ctx: &mut RenderContext<'_>,
    ) {
        let entity = get_string_or(props, "entity", "Unknown");
        let fields = get_fields(props, "fields");

        let mut lines = vec![Line::from(Span::styled(
            format!("{} Form", entity),
            Style::default().add_modifier(Modifier::BOLD),
        ))];

        for field in &fields {
            let value = ctx
                .form_data()
                .get(&field.name)
                .cloned()
                .unwrap_or_default();
            let label = if field.required {
                format!("{}*: ", field.label)
            } else {
                format!("{}: ", field.label)
            };
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(value),
            ]));
        }

        let form = Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL));
        frame.render_widget(form, area);
    }
}

pub struct StatCardRenderer;

impl PatternRenderer for StatCardRenderer {
    fn pattern_name(&self) -> &str {
        "stat-card"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let title = get_string_or(props, "title", "Stat");
        let value = get_string_or(props, "value", "-");
        let subtitle = get_string(props, "subtitle");

        let mut lines = vec![
            Line::from(Span::styled(title, Style::default().fg(Color::Gray))),
            Line::from(Span::styled(
                value,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
        ];
        if let Some(sub) = subtitle {
            lines.push(Line::from(Span::styled(
                sub,
                Style::default().fg(Color::Gray),
            )));
        }

        let card = Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL));
        frame.render_widget(card, area);
    }
}

pub struct MessageRenderer;

impl PatternRenderer for MessageRenderer {
    fn pattern_name(&self) -> &str {
        "message"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let message = get_string_or(props, "message", "");
        let style = if get_bool_or(props, "error", false) {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let paragraph = Paragraph::new(message)
            .style(style)
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }
}

pub struct LoadingStateRenderer;

impl PatternRenderer for LoadingStateRenderer {
    fn pattern_name(&self) -> &str {
        "loading-state"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let message = get_string_or(props, "message", "Loading...");
        frame.render_widget(Paragraph::new(message).alignment(Alignment::Center), area);
    }
}

pub struct EmptyStateRenderer;

impl PatternRenderer for EmptyStateRenderer {
    fn pattern_name(&self) -> &str {
        "empty-state"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let title = get_string_or(props, "title", "Nothing here");
        let message = get_string_or(props, "message", "No data available.");
        let text = Text::from(vec![
            Line::from(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(message),
        ]);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
    }
}

pub struct ErrorStateRenderer;

impl PatternRenderer for ErrorStateRenderer {
    fn pattern_name(&self) -> &str {
        "error-state"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let title = get_string_or(props, "title", "Error");
        let message = get_string_or(props, "message", "Something went wrong.");
        let text = Text::from(vec![
            Line::from(Span::styled(
                title,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(message, Style::default().fg(Color::Red))),
        ]);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
    }
}

/// Renderer for the `button` pattern.
pub struct ButtonRenderer;

impl PatternRenderer for ButtonRenderer {
    fn pattern_name(&self) -> &str {
        "button"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let label = get_string_or(props, "label", "Button");
        let text = Text::from(Line::from(vec![
            Span::styled("[ ", Style::default().fg(Color::DarkGray)),
            Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(" ]", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
            area,
        );
    }
}

/// This renderer's id in the declared unsupported-pattern error.
pub const RENDERER_ID: &str = "rust-tui";

/// The answer for a pattern this target does not implement: the declared
/// unsupported-pattern error (`orbital_core::renderer_coverage`), never a
/// placeholder or a substitute render.
pub struct UnsupportedPatternRenderer;

impl PatternRenderer for UnsupportedPatternRenderer {
    fn pattern_name(&self) -> &str {
        "*"
    }

    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        props: &Value,
        _entity: Option<&Value>,
        _ctx: &mut RenderContext<'_>,
    ) {
        let message = orbital_core::renderer_coverage::UnsupportedPattern::new(
            &get_string_or(props, "type", ""),
            RENDERER_ID,
            env!("CARGO_PKG_VERSION"),
        )
        .message();
        frame.render_widget(
            Paragraph::new(Text::from(Line::from(Span::styled(
                message,
                Style::default().fg(Color::Red),
            )))),
            area,
        );
    }
}
