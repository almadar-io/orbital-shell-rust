//! `orb-shell-rust`: the Rust shell as a program (`orbital_core::shell_protocol`).

use crate::codegen::server::PROGRAM_FILE;
use crate::RustRenderer;
use orbital_core::generate::ShellMode;
use orbital_core::project::write_project;
use orbital_core::shell_protocol::{
    description, written_files, GenerateRequest, GenerateResponse, ShellDescription, ShellFailure,
    ShellOption, ShellProgram, CODE_BAD_REQUEST, CODE_GENERATE_FAILED,
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
        let mut described = description("rust", env!("CARGO_PKG_VERSION"), false, vec![renderer]);
        described.needs_served = true;
        described
    }

    fn generate(
        &self,
        request: &GenerateRequest,
        output: &Path,
    ) -> Result<GenerateResponse, ShellFailure> {
        if renderer(request)? == RustRenderer::Tui {
            return Err(ShellFailure::new(CODE_BAD_REQUEST, TUI_DEFERRED));
        }
        let serves = request.write.mode != ShellMode::Client;
        let program = if serves {
            Some(served_program(request)?)
        } else {
            None
        };
        let generated = write_project(
            &request.module,
            output,
            &crate::RustShell::new(),
            &crate::RustBackend::new(),
            &request.write,
        )?;
        let mut files = written_files(&generated);
        if let Some(program) = program {
            std::fs::write(output.join(PROGRAM_FILE), program).map_err(|e| {
                ShellFailure::new(CODE_GENERATE_FAILED, format!("write {PROGRAM_FILE}: {e}"))
            })?;
            files.push(PROGRAM_FILE.to_string());
            if let Some(source) = &request.source_path {
                orbital_core::project::copy_locales(source, output).map_err(|e| {
                    ShellFailure::new(CODE_GENERATE_FAILED, format!("copy locales: {e}"))
                })?;
            }
        }
        Ok(GenerateResponse {
            files,
            warnings: generated.warnings,
            next_steps: vec![
                format!("cd {}", output.display()),
                "cargo check".to_string(),
                "cargo run".to_string(),
            ],
        })
    }
}

fn served_program(request: &GenerateRequest) -> Result<String, ShellFailure> {
    let served = request.served.as_ref().ok_or_else(|| {
        ShellFailure::new(
            CODE_BAD_REQUEST,
            "the rust shell serves a program on the native kernel, and the request carries no served program",
        )
    })?;
    let serialize = |e: serde_json::Error| {
        ShellFailure::new(
            CODE_GENERATE_FAILED,
            format!("serialize the served program: {e}"),
        )
    };
    let value = serde_json::to_value(served).map_err(serialize)?;
    serde_json::to_string(&orbital_core::util::canonical_json(&value)).map_err(serialize)
}

fn renderer(request: &GenerateRequest) -> Result<RustRenderer, ShellFailure> {
    match request.options.get(RustRenderer::OPTION) {
        None => Ok(RustRenderer::All),
        Some(serde_json::Value::String(v)) => RustRenderer::parse(v).ok_or_else(|| {
            ShellFailure::new(
                CODE_BAD_REQUEST,
                format!(
                    "renderer must be one of {:?}, got {v}",
                    RustRenderer::VALUES
                ),
            )
        }),
        Some(other) => Err(ShellFailure::new(
            CODE_BAD_REQUEST,
            format!("option renderer must be a string, got {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::ir::OirModule;

    fn served(name: &str) -> orbital_core::schema::types::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "typeContractVersion": 1,
            "orbitals": [{
                "name": name,
                "entity": {
                    "name": "E",
                    "persistence": "runtime",
                    "fields": [{ "name": "id", "type": "string" }]
                },
                "traits": [],
                "pages": []
            }]
        }))
        .unwrap()
    }

    fn served_request(
        mode: ShellMode,
        served: Option<orbital_core::schema::types::OrbitalSchema>,
    ) -> GenerateRequest {
        let mut request = request(&[]);
        request.served = served;
        request.write.mode = mode;
        request
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("orb-rust-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn request(options: &[(&str, serde_json::Value)]) -> GenerateRequest {
        GenerateRequest {
            ir_version: orbital_core::shell_protocol::IR_VERSION,
            module: OirModule::new("App", "1.0.0"),
            resolved: None,
            served: None,
            source_path: None,
            write: Default::default(),
            options: options
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        }
    }

    #[test]
    fn renderer_option_defaults_to_all_and_refuses_unknown_values() {
        assert_eq!(renderer(&request(&[])).ok(), Some(RustRenderer::All));
        assert_eq!(
            renderer(&request(&[("renderer", "tui".into())])).ok(),
            Some(RustRenderer::Tui)
        );
        assert!(renderer(&request(&[("renderer", "dioxus".into())])).is_err());
        assert!(renderer(&request(&[("renderer", serde_json::Value::Bool(true))])).is_err());
    }

    #[test]
    fn renderer_tui_is_refused_as_deferred_and_writes_nothing() {
        let out =
            std::env::temp_dir().join(format!("orb-shell-rust-tui-refused-{}", std::process::id()));
        let refused = RustProgram.generate(&request(&[("renderer", "tui".into())]), &out);
        let message = refused.err().map(|f| f.to_string()).unwrap_or_default();
        assert!(
            message.contains("G-CROSS-290") && message.contains("deferred"),
            "{message}"
        );
        assert!(!out.exists(), "nothing is written for a deferred renderer");
    }

    #[test]
    fn describe_advertises_only_the_all_renderer_and_needs_no_resolved_program() {
        let described = RustProgram.describe();
        assert!(!described.needs_resolved);
        let renderer = described
            .options
            .iter()
            .find(|o| o.name == "renderer")
            .expect("renderer option");
        assert_eq!(renderer.values.as_deref(), Some(&["all".to_string()][..]));
    }

    #[test]
    fn describe_asks_for_the_served_program() {
        assert!(RustProgram.describe().needs_served);
    }

    #[test]
    fn a_full_generate_without_a_served_program_is_refused() {
        let failure = RustProgram
            .generate(
                &served_request(ShellMode::Full, None),
                &scratch("full-none"),
            )
            .unwrap_err();
        assert_eq!(failure.code, CODE_BAD_REQUEST);
        assert!(
            failure.message.contains("served program"),
            "{}",
            failure.message
        );
        let server = RustProgram
            .generate(
                &served_request(ShellMode::Server, None),
                &scratch("server-none"),
            )
            .unwrap_err();
        assert_eq!(server.code, CODE_BAD_REQUEST);
    }

    #[test]
    fn control_client_mode_is_not_refused_without_a_served_program() {
        let out = scratch("client-none");
        let result = RustProgram.generate(&served_request(ShellMode::Client, None), &out);
        assert!(result.is_ok(), "{:?}", result.err().map(|f| f.message));
        assert!(!out.join(PROGRAM_FILE).exists());
        let _ = std::fs::remove_dir_all(out);
    }

    #[test]
    fn a_served_program_is_written_as_the_project_program() {
        let out = scratch("served");
        let response = RustProgram
            .generate(
                &served_request(ShellMode::Full, Some(served("ServedApp"))),
                &out,
            )
            .unwrap();
        let program: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join(PROGRAM_FILE)).unwrap())
                .unwrap();
        assert_eq!(program["name"], "ServedApp");
        assert!(response.files.iter().any(|f| f == PROGRAM_FILE));
        let _ = std::fs::remove_dir_all(out);
    }
}
