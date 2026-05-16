use std::path::Path;

use agent_core::facade::{InstructionsUpdateInput, InstructionsView};
use agent_core::CoreError;
use toml_edit::{value, DocumentMut};

/// Read the `instructions` key from a TOML config file.
/// Returns `None` if the file doesn't exist or the key is absent/empty.
pub fn read_instructions(config_path: &Path) -> agent_core::Result<Option<String>> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read config for instructions: {e}"
            )))
        }
    };
    let doc: DocumentMut = raw.parse().map_err(|e| {
        CoreError::InvalidState(format!("failed to parse config for instructions: {e}"))
    })?;
    Ok(doc
        .get("instructions")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

/// Write (or remove) the `instructions` key in a TOML config file.
/// An empty `text` removes the key entirely.
pub fn write_instructions(config_path: &Path, text: &str) -> agent_core::Result<()> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read config for instructions write: {e}"
            )))
        }
    };
    let mut doc: DocumentMut = raw.parse().map_err(|e| {
        CoreError::InvalidState(format!(
            "failed to parse config for instructions write: {e}"
        ))
    })?;

    if text.is_empty() {
        doc.remove("instructions");
    } else {
        doc["instructions"] = value(text);
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CoreError::InvalidState(format!(
                "failed to create config directory for instructions: {e}"
            ))
        })?;
    }
    std::fs::write(config_path, doc.to_string()).map_err(|e| {
        CoreError::InvalidState(format!("failed to write config for instructions: {e}"))
    })?;
    Ok(())
}

pub fn get_system_prompt() -> String {
    crate::agent_loop::SYSTEM_PROMPT.to_string()
}

/// Build an InstructionsView from the system prompt constant and
/// optional user + project instructions.
pub fn build_instructions_view(
    user_instructions: Option<String>,
    project_instructions: Option<String>,
) -> InstructionsView {
    InstructionsView {
        system: get_system_prompt(),
        user: user_instructions,
        project: project_instructions,
    }
}

/// Upsert instructions into the appropriate config file.
pub fn upsert_instructions(
    input: &InstructionsUpdateInput,
    user_config_path: &Path,
    project_config_path: Option<&Path>,
) -> agent_core::Result<()> {
    match input.scope {
        agent_core::ConfigScope::User => write_instructions(user_config_path, &input.text),
        agent_core::ConfigScope::Project => {
            let path = project_config_path.ok_or_else(|| {
                CoreError::InvalidState(
                    "no project config path for project-scoped instructions".into(),
                )
            })?;
            write_instructions(path, &input.text)
        }
        scope => Err(CoreError::InvalidState(format!(
            "instructions can only be set at User or Project scope, got {scope}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config_fixture(raw: &str) -> std::path::PathBuf {
        let file = tempfile::NamedTempFile::new().expect("temp file created");
        let (_file, path) = file.keep().expect("temp file path kept");
        if !raw.is_empty() {
            std::fs::write(&path, raw).expect("fixture written");
        }
        path
    }

    #[test]
    fn reads_instructions_from_config() {
        let path = write_config_fixture(
            "instructions = \"Be concise.\"\n\n[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
        );
        let result = read_instructions(&path).expect("read should succeed");
        assert_eq!(result.as_deref(), Some("Be concise."));
    }

    #[test]
    fn returns_none_when_key_absent() {
        let path = write_config_fixture(
            "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
        );
        let result = read_instructions(&path).expect("read should succeed");
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_when_file_missing() {
        let result = read_instructions(Path::new("/nonexistent/instructions_test.toml"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn writes_instructions_to_config() {
        let path = write_config_fixture(
            "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
        );
        write_instructions(&path, "Use Chinese.").expect("write should succeed");
        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(raw.contains("instructions = \"Use Chinese.\""));
        // Existing content preserved
        assert!(raw.contains("[profiles.fast]"));
        assert!(raw.contains("provider = \"openai_compatible\""));
    }

    #[test]
    fn removes_instructions_with_empty_text() {
        let path = write_config_fixture(
            "instructions = \"old value\"\n[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
        );
        write_instructions(&path, "").expect("write should succeed");
        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(!raw.contains("instructions"));
        assert!(raw.contains("[profiles.fast]"));
    }

    #[test]
    fn creates_new_file_with_instructions() {
        let path = write_config_fixture("");
        write_instructions(&path, "New instructions.").expect("write should succeed");
        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(raw.contains("instructions = \"New instructions.\""));
    }

    #[test]
    fn get_system_prompt_returns_constant() {
        let prompt = get_system_prompt();
        assert!(prompt.contains("Kairox"));
        assert!(prompt.contains("Memory Protocol"));
    }

    #[test]
    fn build_view_concatenates_layers() {
        let view = build_instructions_view(
            Some("User instructions.".into()),
            Some("Project instructions.".into()),
        );
        assert_eq!(view.user.as_deref(), Some("User instructions."));
        assert_eq!(view.project.as_deref(), Some("Project instructions."));
        assert!(view.system.contains("Kairox"));
    }

    #[test]
    fn upsert_user_scope_writes_to_user_config() {
        let path = write_config_fixture("");
        let input = InstructionsUpdateInput {
            scope: agent_core::ConfigScope::User,
            text: "User level instructions.".into(),
        };
        upsert_instructions(&input, &path, None).expect("upsert should succeed");
        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(raw.contains("instructions = \"User level instructions.\""));
    }

    #[test]
    fn upsert_project_scope_requires_project_path() {
        let path = write_config_fixture("");
        let input = InstructionsUpdateInput {
            scope: agent_core::ConfigScope::Project,
            text: "Project instructions.".into(),
        };
        let result = upsert_instructions(&input, &path, Some(&path));
        assert!(result.is_ok());
    }

    #[test]
    fn upsert_rejects_builtin_scope() {
        let path = write_config_fixture("");
        let input = InstructionsUpdateInput {
            scope: agent_core::ConfigScope::Builtin,
            text: "Should fail.".into(),
        };
        let result = upsert_instructions(&input, &path, None);
        assert!(result.is_err());
    }
}
