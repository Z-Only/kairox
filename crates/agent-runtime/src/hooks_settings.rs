use std::path::Path;

use agent_core::facade::{HookSettingsInput, HookSettingsView, HookTemplateView};
use agent_core::CoreError;
use toml_edit::{value, DocumentMut, Item, Table};

pub fn read_hooks_from_config(
    config_path: &Path,
    scope: agent_core::ConfigScope,
) -> agent_core::Result<Vec<HookSettingsView>> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read hooks config: {e}"
            )))
        }
    };
    let config = agent_config::load_from_str(&raw, &config_path.display().to_string())
        .map_err(|e| CoreError::InvalidState(format!("failed to parse hooks config: {e}")))?;

    Ok(config
        .hooks
        .into_iter()
        .map(|hook| HookSettingsView {
            id: hook.id,
            event: hook.event.to_string(),
            matcher: hook.matcher,
            command: hook.command,
            status_message: hook.status_message,
            timeout_secs: hook.timeout_secs.and_then(|value| value.try_into().ok()),
            enabled: hook.enabled,
            source: scope,
            config_path: Some(config_path.display().to_string()),
        })
        .collect())
}

pub fn builtin_hook_templates() -> Vec<HookTemplateView> {
    vec![
        HookTemplateView {
            id: "stop-validation".into(),
            name: "Stop validation".into(),
            description: "Run the workspace test suite after a turn stops.".into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test --workspace --all-targets".into(),
            status_message: Some("Running workspace validation".into()),
            timeout_secs: Some(600),
        },
        HookTemplateView {
            id: "prompt-secret-scan".into(),
            name: "Prompt secret scan".into(),
            description: "Inspect submitted prompts before they enter the model context.".into(),
            event: "UserPromptSubmit".into(),
            matcher: None,
            command: "python3 .kairox/hooks/prompt_secret_scan.py".into(),
            status_message: Some("Scanning prompt".into()),
            timeout_secs: Some(30),
        },
        HookTemplateView {
            id: "pre-tool-policy".into(),
            name: "Pre-tool policy".into(),
            description: "Check tool calls before Kairox asks for permission or executes them."
                .into(),
            event: "PreToolUse".into(),
            matcher: Some("*".into()),
            command: "python3 .kairox/hooks/pre_tool_policy.py".into(),
            status_message: Some("Checking tool policy".into()),
            timeout_secs: Some(30),
        },
    ]
}

pub fn upsert_hook(input: &HookSettingsInput, config_path: &Path) -> agent_core::Result<()> {
    validate_hook_input(input)?;
    let raw = read_config_for_edit(config_path, "hooks write")?;
    let mut doc: DocumentMut = raw
        .parse()
        .map_err(|e| CoreError::InvalidState(format!("failed to parse hooks config: {e}")))?;

    ensure_table(&mut doc, "features");
    ensure_table(&mut doc, "hooks");
    ensure_nested_table(&mut doc, "hooks", &input.event);
    if !doc["hooks"][&input.event]
        .as_table()
        .is_some_and(|table| table.contains_key(&input.id))
        || !doc["hooks"][&input.event][&input.id].is_table()
    {
        doc["hooks"][&input.event][&input.id] = Item::Table(Table::new());
    }

    doc["features"]["hooks"] = value(true);
    let hook = &mut doc["hooks"][&input.event][&input.id];
    hook["command"] = value(input.command.trim());
    hook["enabled"] = value(input.enabled);
    match input
        .matcher
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(matcher) => hook["matcher"] = value(matcher),
        None => {
            hook.as_table_like_mut()
                .and_then(|table| table.remove("matcher"));
        }
    }
    match input
        .status_message
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(status_message) => hook["status_message"] = value(status_message),
        None => {
            hook.as_table_like_mut()
                .and_then(|table| table.remove("status_message"));
        }
    }
    match input.timeout_secs {
        Some(timeout_secs) => hook["timeout_secs"] = value(timeout_secs as i64),
        None => {
            hook.as_table_like_mut()
                .and_then(|table| table.remove("timeout_secs"));
        }
    }

    write_config(config_path, &doc)
}

pub fn delete_hook(config_path: &Path, event: &str, id: &str) -> agent_core::Result<()> {
    if agent_config::HookEvent::parse(event).is_none() {
        return Err(CoreError::InvalidState(format!(
            "unsupported hook event '{event}'"
        )));
    }

    let raw = read_config_for_edit(config_path, "hooks delete")?;
    let mut doc: DocumentMut = raw
        .parse()
        .map_err(|e| CoreError::InvalidState(format!("failed to parse hooks config: {e}")))?;

    if let Some(hooks_table) = doc
        .get_mut("hooks")
        .and_then(|item| item.as_table_like_mut())
    {
        if let Some(event_item) = hooks_table.get_mut(event) {
            if let Some(event_table) = event_item.as_table_like_mut() {
                event_table.remove(id);
                if event_table.is_empty() {
                    hooks_table.remove(event);
                }
            }
        }
        if hooks_table.is_empty() {
            doc.remove("hooks");
        }
    }

    write_config(config_path, &doc)
}

fn validate_hook_input(input: &HookSettingsInput) -> agent_core::Result<()> {
    match input.scope {
        agent_core::ConfigScope::User | agent_core::ConfigScope::Project => {}
        scope => {
            return Err(CoreError::InvalidState(format!(
                "hooks can only be set at User or Project scope, got {scope}"
            )))
        }
    }
    if input.id.trim().is_empty() {
        return Err(CoreError::InvalidState("hook id cannot be empty".into()));
    }
    if agent_config::HookEvent::parse(&input.event).is_none() {
        return Err(CoreError::InvalidState(format!(
            "unsupported hook event '{}'",
            input.event
        )));
    }
    if input.command.trim().is_empty() {
        return Err(CoreError::InvalidState(
            "hook command cannot be empty".into(),
        ));
    }
    Ok(())
}

fn read_config_for_edit(config_path: &Path, operation: &str) -> agent_core::Result<String> {
    match std::fs::read_to_string(config_path) {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(CoreError::InvalidState(format!(
            "failed to read config for {operation}: {e}"
        ))),
    }
}

fn write_config(config_path: &Path, doc: &DocumentMut) -> agent_core::Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CoreError::InvalidState(format!("failed to create hooks config directory: {e}"))
        })?;
    }
    std::fs::write(config_path, doc.to_string())
        .map_err(|e| CoreError::InvalidState(format!("failed to write hooks config: {e}")))?;
    Ok(())
}

fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if !doc.contains_key(key) || !doc[key].is_table() {
        doc[key] = Item::Table(Table::new());
    }
}

fn ensure_nested_table(doc: &mut DocumentMut, parent: &str, key: &str) {
    ensure_table(doc, parent);
    if !doc[parent]
        .as_table()
        .is_some_and(|table| table.contains_key(key))
        || !doc[parent][key].is_table()
    {
        doc[parent][key] = Item::Table(Table::new());
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
    fn upsert_hook_preserves_existing_config() {
        let path = write_config_fixture(
            "instructions = \"Be concise.\"\n\n[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
        );
        let input = HookSettingsInput {
            scope: agent_core::ConfigScope::User,
            id: "verify".into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test --workspace --all-targets".into(),
            status_message: Some("Running tests".into()),
            timeout_secs: Some(120),
            enabled: true,
        };

        upsert_hook(&input, &path).expect("upsert should succeed");

        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(raw.contains("instructions = \"Be concise.\""));
        assert!(raw.contains("[profiles.fast]"));
        assert!(raw.contains("[features]"));
        assert!(raw.contains("hooks = true"));
        assert!(raw.contains("[hooks.Stop.verify]"));
        assert!(raw.contains("command = \"cargo test --workspace --all-targets\""));
    }

    #[test]
    fn delete_hook_removes_empty_event_table() {
        let path = write_config_fixture(
            "[hooks.Stop.verify]\nmatcher = \"*\"\ncommand = \"cargo test\"\nenabled = true\n",
        );

        delete_hook(&path, "Stop", "verify").expect("delete should succeed");

        let raw = std::fs::read_to_string(&path).expect("should read back");
        assert!(!raw.contains("verify"));
        assert!(!raw.contains("Stop"));
        assert!(read_hooks_from_config(&path, agent_core::ConfigScope::User)
            .expect("read should succeed")
            .is_empty());
    }

    #[test]
    fn builtin_templates_include_stop_validation() {
        let templates = builtin_hook_templates();
        let stop_validation = templates
            .iter()
            .find(|template| template.id == "stop-validation")
            .expect("stop validation template should exist");

        assert_eq!(stop_validation.event, "Stop");
        assert!(stop_validation.command.contains("cargo test"));
    }
}
