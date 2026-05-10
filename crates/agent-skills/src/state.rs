use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::types::SkillActivationMode;
use crate::{Result, SkillError};

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillsStateFile {
    #[serde(default)]
    pub skills: BTreeMap<String, SkillStateEntry>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillStateEntry {
    pub enabled: Option<bool>,
    pub activation_mode: Option<SkillActivationMode>,
    pub install_source: Option<String>,
    pub remote: Option<String>,
    pub version: Option<String>,
    pub last_update_check: Option<String>,
    pub update_available: Option<bool>,
}

impl SkillsStateFile {
    pub fn skill(&self, skill_id: &str) -> Option<&SkillStateEntry> {
        self.skills.get(skill_id)
    }

    pub fn skill_mut(&mut self, skill_id: &str) -> &mut SkillStateEntry {
        self.skills.entry(skill_id.to_owned()).or_default()
    }

    pub fn set_enabled(&mut self, skill_id: &str, enabled: bool) {
        self.skill_mut(skill_id).enabled = Some(enabled);
    }
}

pub async fn read_skills_state(path: &Path) -> Result<SkillsStateFile> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(SkillsStateFile::default());
    }

    let raw_state = tokio::fs::read_to_string(path).await?;
    parse_skills_state(&raw_state)
}

pub async fn write_skills_state(path: &Path, state: &SkillsStateFile) -> Result<()> {
    if let Some(parent_directory) = path.parent() {
        tokio::fs::create_dir_all(parent_directory).await?;
    }

    let temporary_path = path.with_extension(format!("tmp-{}", std::process::id()));
    tokio::fs::write(&temporary_path, format_skills_state(state)).await?;
    tokio::fs::rename(temporary_path, path).await?;

    Ok(())
}

fn parse_skills_state(raw_state: &str) -> Result<SkillsStateFile> {
    let mut state = SkillsStateFile::default();
    let mut current_skill_id: Option<String> = None;

    for raw_line in raw_state.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_skill_id = Some(parse_skill_table_header(line)?);
            continue;
        }

        let skill_id = current_skill_id.as_deref().ok_or_else(|| {
            SkillError::InvalidStateFile("state entries must be inside a skill table".to_owned())
        })?;
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| SkillError::InvalidStateFile(format!("invalid state entry: {line}")))?;
        apply_state_value(state.skill_mut(skill_id), key.trim(), value.trim())?;
    }

    Ok(state)
}

fn parse_skill_table_header(line: &str) -> Result<String> {
    let table_name = &line[1..line.len() - 1];
    let Some(skill_id) = table_name.strip_prefix("skills.") else {
        return Err(SkillError::InvalidStateFile(format!(
            "unsupported table: {table_name}"
        )));
    };

    if skill_id.starts_with('"') && skill_id.ends_with('"') {
        return unquote_string(skill_id);
    }

    Ok(skill_id.to_owned())
}

fn apply_state_value(entry: &mut SkillStateEntry, key: &str, value: &str) -> Result<()> {
    match key {
        "enabled" => entry.enabled = Some(parse_bool(value)?),
        "activation_mode" => entry.activation_mode = Some(parse_activation_mode(value)?),
        "install_source" => entry.install_source = Some(unquote_string(value)?),
        "remote" => entry.remote = Some(unquote_string(value)?),
        "version" => entry.version = Some(unquote_string(value)?),
        "last_update_check" => entry.last_update_check = Some(unquote_string(value)?),
        "update_available" => entry.update_available = Some(parse_bool(value)?),
        _ => {}
    }

    Ok(())
}

fn parse_bool(value: &str) -> Result<bool> {
    value.parse::<bool>().map_err(|error| {
        SkillError::InvalidStateFile(format!("invalid boolean value `{value}`: {error}"))
    })
}

fn parse_activation_mode(value: &str) -> Result<SkillActivationMode> {
    match unquote_string(value)?.as_str() {
        "manual" => Ok(SkillActivationMode::Manual),
        "suggest" => Ok(SkillActivationMode::Suggest),
        "auto" => Ok(SkillActivationMode::Auto),
        mode => Err(SkillError::InvalidStateFile(format!(
            "invalid activation mode: {mode}"
        ))),
    }
}

fn unquote_string(value: &str) -> Result<String> {
    let Some(unquoted_value) = value
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    else {
        return Err(SkillError::InvalidStateFile(format!(
            "expected quoted string: {value}"
        )));
    };

    Ok(unquoted_value.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn format_skills_state(state: &SkillsStateFile) -> String {
    let mut output = String::new();

    for (skill_id, entry) in &state.skills {
        output.push_str(&format!("[skills.\"{}\"]\n", quote_string(skill_id)));
        if let Some(enabled) = entry.enabled {
            output.push_str(&format!("enabled = {enabled}\n"));
        }
        if let Some(activation_mode) = entry.activation_mode {
            output.push_str(&format!(
                "activation_mode = \"{}\"\n",
                format_activation_mode(activation_mode)
            ));
        }
        write_optional_string(&mut output, "install_source", &entry.install_source);
        write_optional_string(&mut output, "remote", &entry.remote);
        write_optional_string(&mut output, "version", &entry.version);
        write_optional_string(&mut output, "last_update_check", &entry.last_update_check);
        if let Some(update_available) = entry.update_available {
            output.push_str(&format!("update_available = {update_available}\n"));
        }
        output.push('\n');
    }

    output
}

fn write_optional_string(output: &mut String, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        output.push_str(&format!("{key} = \"{}\"\n", quote_string(value)));
    }
}

fn format_activation_mode(activation_mode: SkillActivationMode) -> &'static str {
    match activation_mode {
        SkillActivationMode::Manual => "manual",
        SkillActivationMode::Suggest => "suggest",
        SkillActivationMode::Auto => "auto",
    }
}

fn quote_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn state_file_persists_disabled_skill_without_touching_skill_markdown() {
        let root = tempfile::tempdir().expect("root should exist");
        let skill_directory = root.path().join("review");
        std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
        let skill_path = skill_directory.join("SKILL.md");
        std::fs::write(
            &skill_path,
            "---\nname: review\ndescription: Review code\n---\nBody\n",
        )
        .expect("skill should be written");

        let state_path = root.path().join("skills-state.toml");
        let mut state = SkillsStateFile::default();
        state.set_enabled("review", false);
        write_skills_state(&state_path, &state)
            .await
            .expect("state should write");

        let reloaded = read_skills_state(&state_path)
            .await
            .expect("state should read");
        assert_eq!(
            reloaded.skill("review").and_then(|entry| entry.enabled),
            Some(false)
        );
        let markdown = std::fs::read_to_string(skill_path).expect("skill markdown should remain");
        assert!(markdown.contains("description: Review code"));
    }
}
