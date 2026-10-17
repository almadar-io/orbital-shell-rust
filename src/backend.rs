//! Rust Backend Implementation
//!
//! Generates Rust source from an Orbital Intermediate Representation module.

use crate::codegen::{
    generate_desktop_bin, generate_effects_module, generate_entities_module, generate_game_bin,
    generate_server_files, generate_services_module, generate_traits, generate_tui_bin,
};
use orbital_core::generate::{Backend, GeneratedFile, GeneratedOutput, ShellMode};
use orbital_core::ir::OirModule;

/// Rust code generation backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustBackend;

impl RustBackend {
    /// Create a new Rust backend.
    pub fn new() -> Self {
        Self
    }
}

impl Backend for RustBackend {
    fn generate(&self, module: &OirModule) -> GeneratedOutput {
        let mut files = vec![];
        let mut warnings = vec![];

        let entities_tokens = generate_entities_module(&module.distinct_entities(), &module.named_types);
        let entities_src = entities_tokens.to_string();
        files.push(GeneratedFile::new("src/entities/mod.rs", entities_src));

        let services_tokens = generate_services_module(&module.services);
        let services_src = services_tokens.to_string();
        files.push(GeneratedFile::new("src/services/mod.rs", services_src));

        // Per-trait state machines.
        for (path, content) in generate_traits(&module.traits) {
            files.push(GeneratedFile::new(path, content));
        }

        // Effect runner and Axum server wiring.
        for (path, content) in generate_effects_module() {
            files.push(GeneratedFile::new(path, content));
        }
        for (path, content) in generate_server_files(module) {
            files.push(GeneratedFile::new(path, content));
        }

        // Native desktop client entry point.
        files.push(GeneratedFile::new(
            "src/bin/desktop.rs",
            generate_desktop_bin(),
        ));

        // Native terminal client entry point.
        files.push(GeneratedFile::new(
            "src/bin/terminal.rs",
            generate_tui_bin(),
        ));

        // Lightweight game client entry point.
        files.push(GeneratedFile::new("src/bin/game.rs", generate_game_bin()));

        // Expose generated modules from the library root.
        files.push(GeneratedFile::new(
            "src/lib.rs",
            generate_lib_rs(&module.name, !module.traits.is_empty()),
        ));

        if module.entities.is_empty() {
            warnings.push("No entities declared in schema.".to_string());
        }
        if module.services.is_empty() {
            warnings.push("No services declared in schema.".to_string());
        }
        if module.traits.is_empty() {
            warnings.push("No traits declared in schema.".to_string());
        }

        GeneratedOutput { files, warnings }
    }

    fn generate_with_mode(&self, module: &OirModule, mode: ShellMode) -> GeneratedOutput {
        match mode {
            ShellMode::Full => self.generate(module),
            ShellMode::Server => {
                let mut files = vec![];
                let mut warnings = vec![];

                let entities_tokens = generate_entities_module(&module.distinct_entities(), &module.named_types);
                files.push(GeneratedFile::new(
                    "src/entities/mod.rs",
                    entities_tokens.to_string(),
                ));

                let services_tokens = generate_services_module(&module.services);
                files.push(GeneratedFile::new(
                    "src/services/mod.rs",
                    services_tokens.to_string(),
                ));

                for (path, content) in generate_traits(&module.traits) {
                    files.push(GeneratedFile::new(path, content));
                }

                for (path, content) in generate_effects_module() {
                    files.push(GeneratedFile::new(path, content));
                }
                for (path, content) in generate_server_files(module) {
                    files.push(GeneratedFile::new(path, content));
                }

                files.push(GeneratedFile::new(
                    "src/lib.rs",
                    generate_lib_rs(&module.name, !module.traits.is_empty()),
                ));

                if module.entities.is_empty() {
                    warnings.push("No entities declared in schema.".to_string());
                }

                GeneratedOutput { files, warnings }
            }
            ShellMode::Client => {
                let mut files = vec![];
                let mut warnings = vec![];

                let entities_tokens = generate_entities_module(&module.distinct_entities(), &module.named_types);
                files.push(GeneratedFile::new(
                    "src/entities/mod.rs",
                    entities_tokens.to_string(),
                ));

                for (path, content) in generate_traits(&module.traits) {
                    files.push(GeneratedFile::new(path, content));
                }

                files.push(GeneratedFile::new(
                    "src/bin/desktop.rs",
                    generate_desktop_bin(),
                ));
                files.push(GeneratedFile::new(
                    "src/bin/terminal.rs",
                    generate_tui_bin(),
                ));
                files.push(GeneratedFile::new("src/bin/game.rs", generate_game_bin()));

                files.push(GeneratedFile::new(
                    "src/lib.rs",
                    generate_lib_rs(&module.name, !module.traits.is_empty()),
                ));

                if module.traits.is_empty() {
                    warnings.push("No traits declared in schema.".to_string());
                }

                GeneratedOutput { files, warnings }
            }
        }
    }

    fn name(&self) -> &'static str {
        "rust"
    }
}

fn generate_lib_rs(app_name: &str, has_traits: bool) -> String {
    let traits_mod = if has_traits { "pub mod traits;\n" } else { "" };
    format!(
        r#"//! {}
//!
//! Generated by the Orbital Rust shell.

pub mod effects;
pub mod entities;
pub mod server;
pub mod services;
{}
"#,
        app_name, traits_mod
    )
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

    /// Kernel-mode shell: the generated state machines carry no effect bodies, so an `os/` effect is
    /// never half-emitted. It is unsupported in generated code by design and runs through the
    /// `EffectRunner` seam only.
    #[test]
    fn os_effects_are_not_silently_dropped_by_the_rust_shell() {
        use orbital_core::ir::{OirEffect, OirExpr, OirState, OirTrait, OirTransition};
        let effect = OirEffect::os(
            orbital_core::runtime::os::OsOp::ReadFile,
            vec![OirExpr::string("/data/secret.txt")],
            None,
            None,
        );
        assert!(!orbital_core::effects::CLIENT_EFFECT_KINDS.contains(&effect.kind()));
        let mut module = test_module();
        let mut t = OirTrait::new("Files");
        t.states.push(OirState::initial("idle"));
        let mut tr = OirTransition::new("idle", "idle", "READ");
        tr.effects.push(OirExpr::effect(effect));
        t.transitions.push(tr);
        module.traits.push(t);
        let output = RustBackend::new().generate(&module);
        let all: String = output.files.iter().map(|f| f.content.as_str()).collect();
        assert!(
            !all.contains("secret.txt") && !all.contains("read-file"),
            "os/ is unsupported in generated Rust: it must not be half-emitted"
        );
        assert!(all.contains("run_effects"), "effects must stay behind the EffectRunner seam");
    }

    #[test]
    fn test_backend_name() {
        let backend = RustBackend::new();
        assert_eq!(backend.name(), "rust");
    }

    #[test]
    fn test_generate_produces_files() {
        let backend = RustBackend::new();
        let output = backend.generate(&test_module());
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/entities/mod.rs"));
        assert!(paths.contains(&"src/services/mod.rs"));
        assert!(paths.contains(&"src/effects/mod.rs"));
        assert!(paths.contains(&"src/server.rs"));
        assert!(paths.contains(&"src/main.rs"));
        assert!(paths.contains(&"src/lib.rs"));
    }

    #[test]
    fn server_mode_omits_client_bins() {
        let backend = RustBackend::new();
        let output = backend.generate_with_mode(&test_module(), ShellMode::Server);
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();

        // Server mode must include server files.
        assert!(paths.contains(&"src/entities/mod.rs"));
        assert!(paths.contains(&"src/effects/mod.rs"));
        assert!(paths.contains(&"src/server.rs"));
        assert!(paths.contains(&"src/main.rs"));

        // Server mode must NOT include native client entry points.
        assert!(!paths.contains(&"src/bin/desktop.rs"));
        assert!(!paths.contains(&"src/bin/terminal.rs"));
        assert!(!paths.contains(&"src/bin/game.rs"));
    }

    #[test]
    fn client_mode_omits_server_files() {
        let backend = RustBackend::new();
        let output = backend.generate_with_mode(&test_module(), ShellMode::Client);
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();

        // Client mode must include the native client entry points.
        assert!(paths.contains(&"src/bin/desktop.rs"));
        assert!(paths.contains(&"src/bin/terminal.rs"));
        assert!(paths.contains(&"src/bin/game.rs"));

        // Client mode must NOT include server or effects files.
        assert!(!paths.contains(&"src/server.rs"));
        assert!(!paths.contains(&"src/main.rs"));
        assert!(!paths.contains(&"src/effects/mod.rs"));
    }

    #[test]
    fn full_mode_matches_generate() {
        let backend = RustBackend::new();
        let module = test_module();
        let full_output = backend.generate_with_mode(&module, ShellMode::Full);
        let direct_output = backend.generate(&module);
        let full_paths: std::collections::HashSet<_> =
            full_output.files.iter().map(|f| f.path.as_str()).collect();
        let direct_paths: std::collections::HashSet<_> =
            direct_output.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(full_paths, direct_paths);
    }
}
