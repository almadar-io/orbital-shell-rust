//! Rust Shell for Orbital Compiler
//!
//! Provides the `Shell` and `Backend` implementations that turn an Orbital
//! Intermediate Representation (`OirModule`) into a native Rust Cargo project.

mod backend;
pub mod codegen;
mod shell;

pub use backend::RustBackend;
pub use shell::RustShell;
