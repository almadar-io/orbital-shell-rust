//! Rust Shell Implementation
//!
//! Implements the `Shell` trait for Rust/Cargo projects.

use orbital_core::shell::{Shell, ShellError};
use orbital_core::str_utils::to_title_case;
use std::path::{Path, PathBuf};

/// Rust shell for native Cargo projects.
pub struct RustShell {
    template_path: PathBuf,
}

impl RustShell {
    /// Create a new Rust shell, locating the template at runtime.
    pub fn new() -> Self {
        let template_path = Self::find_template_path();
        Self { template_path }
    }

    /// Find the `rust-shell` template directory at runtime.
    fn find_template_path() -> PathBuf {
        // The one template lookup (`ORBITAL_SHELLS_DIR` → beside the binary → the checkout).
        orbital_core::renderer_packs::shell_template("rust-shell", std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    /// Create with an explicit template path.
    pub fn with_template_path(template_path: impl Into<PathBuf>) -> Self {
        Self {
            template_path: template_path.into(),
        }
    }
}

impl Default for RustShell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell for RustShell {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn description(&self) -> &'static str {
        "Rust native application with Axum server and egui client"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rs"]
    }

    fn template_path(&self) -> &Path {
        &self.template_path
    }

    fn generated_dir(&self) -> &str {
        "."
    }

    fn build_command(&self) -> &str {
        "cargo build --release"
    }

    fn dev_command(&self) -> &str {
        "cargo run"
    }

    fn test_command(&self) -> &str {
        "cargo test"
    }

    fn validate_structure(&self, output_dir: &Path) -> Result<(), ShellError> {
        let required_files = ["Cargo.toml", "src/main.rs"];
        for file in &required_files {
            let path = output_dir.join(file);
            if !path.exists() {
                return Err(ShellError::InvalidStructure {
                    path: path.clone(),
                    message: format!("Missing required file: {}", file),
                });
            }
        }
        Ok(())
    }

    fn copy_template(&self, output_dir: &Path) -> Result<(), ShellError> {
        if !self.template_path.exists() {
            return Err(ShellError::NotFound(
                self.template_path.display().to_string(),
            ));
        }
        copy_dir_recursive(&self.template_path, output_dir)?;

        // Replace placeholders in text files so the emitted Cargo project has a
        // valid package name derived from the output directory.
        let project_name = output_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "orbital-app".to_string());
        let replacements = build_replacements(&project_name);
        replace_placeholders_in_dir(output_dir, &replacements)?;

        Ok(())
    }

    fn init_project(&self, target_dir: &Path, project_name: &str) -> Result<(), ShellError> {
        self.copy_template(target_dir)?;
        let gen_dir = target_dir.join(self.generated_dir());
        std::fs::create_dir_all(&gen_dir)?;
        let _ = project_name;
        Ok(())
    }
}

/// Recursively copy a directory, skipping generated artifacts.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ShellError> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str == "generated"
            || file_name_str == "target"
            || file_name_str == ".git"
            || file_name_str == "Cargo.lock"
        {
            continue;
        }

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Build placeholder replacement map from a project name.
fn build_replacements(project_name: &str) -> std::collections::HashMap<&'static str, String> {
    let mut map = std::collections::HashMap::new();
    map.insert("{{APP_NAME}}", project_name.to_string());
    map.insert("{{APP_TITLE}}", to_title_case(project_name));
    map
}

/// Replace placeholders in all text files under `dir`.
fn replace_placeholders_in_dir(
    dir: &Path,
    replacements: &std::collections::HashMap<&str, String>,
) -> Result<(), ShellError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            replace_placeholders_in_dir(&path, replacements)?;
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_text = matches!(ext, "rs" | "toml" | "md" | "txt");
        if !is_text {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let mut updated = content;
        for (placeholder, value) in replacements {
            updated = updated.replace(placeholder, value);
        }
        std::fs::write(&path, updated)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_properties() {
        let shell = RustShell::new();
        assert_eq!(shell.name(), "rust");
        assert_eq!(shell.extensions(), &["rs"]);
        assert_eq!(shell.generated_dir(), ".");
    }
}
