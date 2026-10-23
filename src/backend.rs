//! Rust Backend Implementation
//!
//! Generates Rust source from an Orbital Intermediate Representation module.

use crate::codegen::{
    generate_desktop_bin, generate_game_bin, generate_server_main, generate_tui_bin,
    generate_tui_standalone_main,
};
use orbital_core::generate::{Backend, GeneratedFile, GeneratedOutput, ShellMode};
use orbital_core::ir::OirModule;

/// Which renderer set the generated project carries (the shell's `renderer` option).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RustRenderer {
    /// The native host entry plus the desktop, terminal and game clients.
    #[default]
    All,
    /// One standalone terminal binary: the resolved program embedded, the kernel in-process, no server.
    Tui,
}

impl RustRenderer {
    pub const OPTION: &'static str = "renderer";
    pub const VALUES: [&'static str; 2] = ["all", "tui"];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "tui" => Some(Self::Tui),
            _ => None,
        }
    }
}

/// Rust code generation backend.
#[derive(Debug, Default, Clone)]
pub struct RustBackend {
    /// The resolved program JSON a standalone terminal binary embeds; `None` generates the full project.
    terminal_program: Option<String>,
}

impl RustBackend {
    /// Create a new Rust backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// The standalone terminal backend, embedding `program` (the resolved `.orb` JSON).
    pub fn terminal(program: String) -> Self {
        Self {
            terminal_program: Some(program),
        }
    }
}

fn generate_terminal(program: &str) -> GeneratedOutput {
    GeneratedOutput {
        files: vec![
            GeneratedFile::new("program.orb", program.to_string()),
            GeneratedFile::new("src/main.rs", generate_tui_standalone_main()),
        ],
        warnings: vec![],
    }
}

fn client_bins() -> Vec<GeneratedFile> {
    vec![
        GeneratedFile::new("src/bin/desktop.rs", generate_desktop_bin()),
        GeneratedFile::new("src/bin/terminal.rs", generate_tui_bin()),
        GeneratedFile::new("src/bin/game.rs", generate_game_bin()),
    ]
}

impl Backend for RustBackend {
    fn generate(&self, module: &OirModule) -> GeneratedOutput {
        self.generate_with_mode(module, ShellMode::Full)
    }

    fn generate_with_mode(&self, _module: &OirModule, mode: ShellMode) -> GeneratedOutput {
        if let Some(program) = &self.terminal_program {
            return generate_terminal(program);
        }
        let mut files = vec![];
        if mode != ShellMode::Client {
            files.push(GeneratedFile::new("src/main.rs", generate_server_main()));
        }
        if mode != ShellMode::Server {
            files.extend(client_bins());
        }
        GeneratedOutput {
            files,
            warnings: vec![],
        }
    }

    fn name(&self) -> &'static str {
        "rust"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::{OirEntity, OirField, OirModule, OirType};

    fn test_module() -> OirModule {
        let mut module = OirModule::new("TestApp", "1.0.0");
        let mut entity = OirEntity::new("User");
        entity.fields.push(OirField::new("email", OirType::String));
        module.entities.push(entity);
        module
    }

    #[test]
    fn test_backend_name() {
        let backend = RustBackend::new();
        assert_eq!(backend.name(), "rust");
    }

    #[test]
    fn test_generate_produces_the_host_entry_and_the_clients() {
        let output = RustBackend::new().generate(&test_module());
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "src/main.rs",
                "src/bin/desktop.rs",
                "src/bin/terminal.rs",
                "src/bin/game.rs"
            ]
        );
    }

    #[test]
    fn server_mode_omits_client_bins() {
        let output = RustBackend::new().generate_with_mode(&test_module(), ShellMode::Server);
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/main.rs"]);
    }

    #[test]
    fn client_mode_omits_the_host_entry() {
        let output = RustBackend::new().generate_with_mode(&test_module(), ShellMode::Client);
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "src/bin/desktop.rs",
                "src/bin/terminal.rs",
                "src/bin/game.rs"
            ]
        );
    }

    #[test]
    fn tui_renderer_emits_one_standalone_binary_with_the_program_embedded() {
        use crate::{RustRenderer, RustShell};
        use orbital_core::project::{write_project, WriteOptions};
        let program = r#"{"name":"TestApp","orbitals":[]}"#;
        let output = std::env::temp_dir().join(format!("orb-agent-tui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output);
        let options = WriteOptions {
            copy_template: true,
            service_runtimes: Some(Default::default()),
            ..Default::default()
        };
        let generated = write_project(
            &test_module(),
            &output,
            &RustShell::for_renderer(RustRenderer::Tui),
            &RustBackend::terminal(program.to_string()),
            &options,
        )
        .expect("terminal project written");
        let paths: Vec<&str> = generated.files.iter().map(|f| f.path.as_str()).collect();
        assert!(
            !paths.iter().any(|p| p.starts_with("src/bin/")),
            "one binary only: {paths:?}"
        );
        assert!(!paths.contains(&"src/server.rs"));

        let read = |rel: &str| {
            std::fs::read_to_string(output.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
        };
        let manifest = read("Cargo.toml");
        for banned in ["dioxus", "wgpu", "orbital-server", "axum", "[[bin]]"] {
            assert!(
                !manifest.contains(banned),
                "terminal manifest must not carry {banned}:\n{manifest}"
            );
        }
        assert!(manifest.contains("orbital-client-tui"));
        let main = read("src/main.rs");
        assert!(
            main.contains("include_str!(\"../program.orb\")"),
            "program embedded:\n{main}"
        );
        assert!(
            !main.contains("resolved.orb")
                && !main.contains("SERVER_URL")
                && !main.contains("3030")
        );
        assert_eq!(read("program.orb"), program);
        let _ = std::fs::remove_dir_all(&output);
    }

    #[test]
    fn full_mode_matches_generate() {
        let backend = RustBackend::new();
        let module = test_module();
        let full_output = backend.generate_with_mode(&module, ShellMode::Full);
        let direct_output = backend.generate(&module);
        let full_paths: std::collections::HashSet<_> =
            full_output.files.iter().map(|f| f.path.as_str()).collect();
        let direct_paths: std::collections::HashSet<_> = direct_output
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(full_paths, direct_paths);
    }
}
