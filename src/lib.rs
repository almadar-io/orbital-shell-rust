//! Rust Shell for Orbital Compiler
//!
//! Provides the `Shell` and `Backend` implementations that turn an Orbital
//! Intermediate Representation (`OirModule`) into a native Rust Cargo project: the host entry for the served
//! program and the client entry points.

mod backend;
pub mod codegen;
#[path = "../hosts/native/src/lib.rs"]
pub mod host;
mod shell;
pub mod shell_program;

pub use backend::{RustBackend, RustRenderer};
pub use shell::RustShell;

#[cfg(feature = "desktop")]
#[path = "../hosts/desktop/src/lib.rs"]
pub mod desktop;
