use crate::facade_runtime::LocalRuntime;
use agent_core::facade::{ProfileSettingsInput, ProfileSettingsView};
use agent_store::EventStore;
// ── Inherent methods ──────────────────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        let profiles_toml_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?;
        let user_config_path = std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h)
                .join(".kairox")
                .join("config.toml")
        });
        let project_config_path = std::env::current_dir()
            .ok()
            .map(|d| d.join(".kairox").join("config.toml"));
        crate::profile_settings::list_profile_settings(
            &self.config,
            profiles_toml_path.as_deref(),
            user_config_path.as_deref(),
            project_config_path.as_deref(),
            source_filter.as_deref(),
        )
        .await
    }

    pub(crate) async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> agent_core::Result<ProfileSettingsView> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::upsert_profile_settings_in_file(&config_path, &input).await
    }

    pub(crate) async fn set_profile_enabled(
        &self,
        alias: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::set_profile_enabled_in_file(
            &config_path,
            &alias,
            enabled,
            &self.config,
        )
        .await
    }

    pub(crate) async fn delete_profile_settings(&self, alias: String) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::delete_profile_in_file(&config_path, &alias).await
    }

    pub(crate) async fn move_profile_in_order(
        &self,
        alias: String,
        direction: i32,
    ) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot reorder profiles".into(),
            )
        })?;
        crate::profile_settings::move_profile_in_order(&config_path, &alias, direction).await
    }

    pub(crate) async fn open_config_dir(&self) -> agent_core::Result<Option<String>> {
        Ok(self
            .marketplace_dir
            .as_ref()
            .map(|p| p.display().to_string()))
    }

    pub(crate) async fn open_profiles_config_file(&self) -> agent_core::Result<Option<String>> {
        Ok(
            crate::profile_settings::writable_profiles_config_path(
                self.marketplace_dir.as_deref(),
            )?
            .map(|path| path.display().to_string()),
        )
    }
}
