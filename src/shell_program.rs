//! `orb-shell-rust`: the Rust shell as a program (`orbital_core::shell_protocol`).

use orbital_core::project::write_project;
use orbital_core::shell_protocol::{
    description, written_files, GenerateRequest, GenerateResponse, ShellDescription, ShellFailure, ShellProgram,
};
use std::path::Path;

pub struct RustProgram;

impl ShellProgram for RustProgram {
    fn describe(&self) -> ShellDescription {
        description("rust", env!("CARGO_PKG_VERSION"), false, Vec::new())
    }

    fn generate(&self, request: &GenerateRequest, output: &Path) -> Result<GenerateResponse, ShellFailure> {
        let generated = write_project(&request.module, output, &crate::RustShell::new(), &crate::RustBackend::new(), &request.write)?;
        Ok(GenerateResponse {
            files: written_files(&generated),
            warnings: generated.warnings,
            next_steps: vec![format!("cd {}", output.display()), "cargo check".to_string(), "cargo run".to_string()],
        })
    }
}
