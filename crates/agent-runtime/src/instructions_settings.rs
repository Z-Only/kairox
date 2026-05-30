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
#[path = "instructions_settings_tests.rs"]
mod tests;
