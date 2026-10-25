use super::{PatternRenderer, TuiApp};
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::client_core::ClientCore;
use orbital_core::client::client_kernel::ClientKernel;
use orbital_core::client::http::SyncServerBridge;
use orbital_core::OrbitalSchema;
use std::sync::Arc;
use tracing::info;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::path::Path;
use std::time::Duration;

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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
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


/// The transport for a terminal app: a server at `server_url`, else the
/// in-process kernel (offline).
fn bridge(
    schema: &OrbitalSchema,
    server_url: Option<&str>,
) -> anyhow::Result<Arc<dyn ServerBridge>> {
    Ok(match server_url {
        Some(url) => Arc::new(SyncServerBridge::new(url)),
        None => {
            let kernel = KernelBridge::new(schema.clone())
                .map_err(|e| anyhow::anyhow!("kernel bridge: {e}"))?;
            Arc::new(kernel)
        }
    })
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

}
