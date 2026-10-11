//! Rust code generation from Orbital Intermediate Representation.

pub mod desktop;
pub mod effects;
pub mod entities;
pub mod expr;
pub mod game;
pub mod server;
pub mod services;
pub mod state_machine;
pub mod tui;

pub use desktop::generate_desktop_bin;
pub use effects::generate_effects_module;
pub use entities::generate_entities_module;
pub use expr::compile_guard_with_fallback;
pub use game::generate_game_bin;
pub use server::generate_server_files;
pub use services::generate_services_module;
pub use state_machine::generate_traits;
pub use tui::generate_tui_bin;
