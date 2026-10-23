//! Rust code generation for the shell's client entry points and the native host entry.

pub mod desktop;
pub mod game;
pub mod server;
pub mod tui;

pub use desktop::generate_desktop_bin;
pub use game::generate_game_bin;
pub use server::generate_server_main;
pub use tui::{generate_tui_bin, generate_tui_standalone_main};
