use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::types::SkillActivationMode;
use crate::{Result, SkillError};

static NEXT_TEMPORARY_FILE_SUFFIX: AtomicU64 = AtomicU64::new(0);

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

    let temporary_path = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        NEXT_TEMPORARY_FILE_SUFFIX.fetch_add(1, Ordering::Relaxed)
    ));
    tokio::fs::write(&temporary_path, format_skills_state(state)?).await?;
    tokio::fs::rename(temporary_path, path).await?;

    Ok(())
}

fn parse_skills_state(raw_state: &str) -> Result<SkillsStateFile> {
    toml::from_str(raw_state).map_err(|error| SkillError::InvalidStateFile(error.to_string()))
}

fn format_skills_state(state: &SkillsStateFile) -> Result<String> {
    toml::to_string_pretty(state).map_err(|error| SkillError::InvalidStateFile(error.to_string()))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
