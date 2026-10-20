//! `orb-shell-rust`: the Rust shell as a program (`orbital_core::shell_protocol`).

use orbital_core::project::write_project;
use crate::RustRenderer;
use orbital_core::shell_protocol::{
    description, written_files, GenerateRequest, GenerateResponse, ShellDescription, ShellFailure, ShellOption,
    ShellProgram, CODE_BAD_REQUEST,
};
use std::path::Path;

pub struct RustProgram;

const TUI_DEFERRED: &str =
    "renderer tui: the terminal renderer is deferred (G-CROSS-290); its standalone binary has no run_program entry yet";

impl ShellProgram for RustProgram {
    fn describe(&self) -> ShellDescription {
        let renderer = ShellOption {
            name: RustRenderer::OPTION.to_string(),
            description: "Renderer set: every client plus the server".to_string(),
            values: Some(vec!["all".to_string()]),
            default: Some("all".to_string()),
        };
        description("rust", env!("CARGO_PKG_VERSION"), false, vec![renderer])
    }

    fn generate(&self, request: &GenerateRequest, output: &Path) -> Result<GenerateResponse, ShellFailure> {
        if renderer(request)? == RustRenderer::Tui {
            return Err(ShellFailure::new(CODE_BAD_REQUEST, TUI_DEFERRED));
        }
        let generated = write_project(&request.module, output, &crate::RustShell::new(), &crate::RustBackend::new(), &request.write)?;
        Ok(GenerateResponse {
            files: written_files(&generated),
            warnings: generated.warnings,
            next_steps: vec![format!("cd {}", output.display()), "cargo check".to_string(), "cargo run".to_string()],
        })
    }
}

fn renderer(request: &GenerateRequest) -> Result<RustRenderer, ShellFailure> {
    match request.options.get(RustRenderer::OPTION) {
        None => Ok(RustRenderer::All),
        Some(serde_json::Value::String(v)) => RustRenderer::parse(v).ok_or_else(|| {
            ShellFailure::new(CODE_BAD_REQUEST, format!("renderer must be one of {:?}, got {v}", RustRenderer::VALUES))
        }),
        Some(other) => Err(ShellFailure::new(CODE_BAD_REQUEST, format!("option renderer must be a string, got {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::OirModule;

    fn request(options: &[(&str, serde_json::Value)]) -> GenerateRequest {
        GenerateRequest {
            ir_version: orbital_core::shell_protocol::IR_VERSION,
            module: OirModule::new("App", "1.0.0"),
            resolved: None,
            source_path: None,
            write: Default::default(),
            options: options.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        }
    }

    #[test]
    fn renderer_option_defaults_to_all_and_refuses_unknown_values() {
        assert_eq!(renderer(&request(&[])).ok(), Some(RustRenderer::All));
        assert_eq!(renderer(&request(&[("renderer", "tui".into())])).ok(), Some(RustRenderer::Tui));
        assert!(renderer(&request(&[("renderer", "dioxus".into())])).is_err());
        assert!(renderer(&request(&[("renderer", serde_json::Value::Bool(true))])).is_err());
    }

    #[test]
    fn renderer_tui_is_refused_as_deferred_and_writes_nothing() {
        let out = std::env::temp_dir().join(format!("orb-shell-rust-tui-refused-{}", std::process::id()));
        let refused = RustProgram.generate(&request(&[("renderer", "tui".into())]), &out);
        let message = refused.err().map(|f| f.to_string()).unwrap_or_default();
        assert!(message.contains("G-CROSS-290") && message.contains("deferred"), "{message}");
        assert!(!out.exists(), "nothing is written for a deferred renderer");
    }

    #[test]
    fn describe_advertises_only_the_all_renderer_and_needs_no_resolved_program() {
        let described = RustProgram.describe();
        assert!(!described.needs_resolved);
        let renderer = described.options.iter().find(|o| o.name == "renderer").expect("renderer option");
        assert_eq!(renderer.values.as_deref(), Some(&["all".to_string()][..]));
    }
}
