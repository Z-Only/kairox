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
        self.list_profile_settings_for_project(source_filter, None)
            .await
    }

    pub(crate) async fn list_profile_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        let profiles_toml_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?;
        let user_config_path = std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h)
                .join(".kairox")
                .join("config.toml")
        });
        let project_config_path = project_root
            .map(std::path::PathBuf::from)
            .map(|root| root.join(".kairox").join("config.toml"))
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| d.join(".kairox").join("config.toml"))
            });
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
        let mut order = self
            .list_profile_settings(None)
            .await?
            .into_iter()
            .map(|profile| profile.alias)
            .collect::<Vec<_>>();

        if let Some(pos) = order
            .iter()
            .position(|profile_alias| profile_alias == &alias)
        {
            let new_pos = if direction < 0 {
                pos.saturating_sub(1)
            } else {
                (pos + 1).min(order.len().saturating_sub(1))
            };
            if new_pos != pos {
                order.swap(pos, new_pos);
            }
        } else {
            order.push(alias);
        }

        crate::profile_settings::save_profile_display_order(&config_path, &order).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::AppFacade;
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;
    use std::sync::Arc;

    fn config_with_three_profiles() -> Arc<agent_config::Config> {
        Arc::new(
            agent_config::load_from_str(
                r#"
[profiles.alpha]
provider = "fake"
model_id = "alpha"

[profiles.bravo]
provider = "fake"
model_id = "bravo"

[profiles.charlie]
provider = "fake"
model_id = "charlie"
"#,
                "test.toml",
            )
            .expect("config should parse"),
        )
    }

    #[tokio::test]
    async fn move_profile_in_order_uses_current_display_order_for_unordered_profiles() {
        let config_dir = tempfile::tempdir().expect("config dir");
        let store = SqliteEventStore::in_memory()
            .await
            .expect("in-memory store");
        let runtime = LocalRuntime::new(store, FakeModelClient::new(vec!["ok".into()]))
            .with_config(config_with_three_profiles())
            .with_marketplace(config_dir.path().to_path_buf())
            .expect("marketplace wiring");

        runtime
            .move_profile_in_order("charlie".into(), -1)
            .await
            .expect("profile should move up");

        let aliases = AppFacade::list_profile_settings(&runtime, None)
            .await
            .expect("profiles should list")
            .into_iter()
            .map(|profile| profile.alias)
            .filter(|alias| ["alpha", "bravo", "charlie"].contains(&alias.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(aliases, vec!["alpha", "charlie", "bravo"]);
    }
}
