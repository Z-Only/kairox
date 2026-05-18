use std::collections;

use agent_core::{
    EventPayload, InstallOutcomeView as CoreInstallOutcomeView,
    InstallRequest as CoreInstallRequest, InstalledEntry as CoreInstalledEntry,
};
use agent_mcp::installer::InstallOutcomeView;
use agent_mcp::types::{McpServerDef, McpTransportDef};
use agent_mcp::InstallSpec;

use super::emit_marketplace_event;
use crate::facade_runtime::LocalRuntime;
use agent_store::EventStore;

// ── Marketplace install / uninstall ─────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn install_catalog_entry(
        &self,
        request: CoreInstallRequest,
    ) -> agent_core::Result<CoreInstallOutcomeView> {
        let catalog = self.catalog.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;

        let inner_req = map_install_request(request);
        let entry = catalog
            .get(&inner_req.catalog_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog: {e}")))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "entry not found: {}",
                    inner_req.catalog_id
                ))
            })?;

        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryInstalling {
                catalog_id: inner_req.catalog_id.clone(),
                source: inner_req.source.clone(),
            },
        );

        let outcome = installer
            .install(&entry, &inner_req)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        match &outcome {
            InstallOutcomeView::RuntimeMissing { missing } => {
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogRuntimeMissing {
                        catalog_id: inner_req.catalog_id.clone(),
                        missing: missing.iter().map(|r| r.kind.as_str().into()).collect(),
                    },
                );
            }
            InstallOutcomeView::Installed { server_id, started } => {
                if let Some(manager) = &self.mcp_manager {
                    let def = build_server_def(&entry, &inner_req);
                    let mut mgr = manager.lock().await;
                    if !mgr.is_registered(server_id) {
                        if let Err(e) = mgr.register_dynamic(def) {
                            tracing::warn!(
                                "marketplace install: register_dynamic({server_id}) failed: {e}"
                            );
                        }
                    }
                    if *started {
                        if let Err(e) = mgr.ensure_server(server_id).await {
                            tracing::warn!(
                                "marketplace install: ensure_server({server_id}) failed: {e}"
                            );
                        }
                    }
                }
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogEntryInstalled {
                        catalog_id: inner_req.catalog_id.clone(),
                        source: inner_req.source.clone(),
                        server_id: server_id.clone(),
                    },
                );
            }
            _ => {}
        }
        Ok(map_outcome_to_core(outcome))
    }

    pub(crate) async fn uninstall_catalog_entry(
        &self,
        server_id: String,
    ) -> agent_core::Result<()> {
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot uninstall".into(),
            )
        })?;
        installer
            .uninstall(&server_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;
        if let Some(manager) = &self.mcp_manager {
            if let Err(e) = manager.lock().await.unregister_dynamic(&server_id).await {
                tracing::warn!(
                    "marketplace uninstall: unregister_dynamic({server_id}) failed: {e}"
                );
            }
        }
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryUninstalled {
                server_id: server_id.clone(),
            },
        );
        Ok(())
    }

    pub(crate) async fn list_installed_entries(
        &self,
    ) -> agent_core::Result<Vec<CoreInstalledEntry>> {
        let Some(installer) = self.installer.as_ref() else {
            return Ok(Vec::new());
        };
        let records = installer
            .list_installed_records()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        let mut out = Vec::with_capacity(records.len());
        for record in records {
            let server_id = record.server_id;
            let catalog_lookup_id = record.catalog_id.as_deref().unwrap_or(&server_id);
            let entry = if let Some(c) = &self.catalog {
                c.get(catalog_lookup_id).await.ok().flatten()
            } else {
                None
            };
            let running = if let Some(manager) = &self.mcp_manager {
                manager.lock().await.is_running(&server_id).unwrap_or(false)
            } else {
                false
            };
            let display_name = entry
                .as_ref()
                .map(|e| e.display_name.clone())
                .unwrap_or_else(|| server_id.clone());
            out.push(CoreInstalledEntry {
                server_id,
                catalog_id: entry.as_ref().map(|e| e.id.clone()).or(record.catalog_id),
                source: entry.as_ref().map(|e| e.source.clone()).or(record.source),
                display_name,
                installed_at: chrono::Utc::now().to_rfc3339(),
                running,
            });
        }
        Ok(out)
    }
}

// ── Install mapping helpers ─────────────────────────────────────────────────

fn map_install_request(r: CoreInstallRequest) -> agent_mcp::catalog::InstallRequest {
    agent_mcp::catalog::InstallRequest {
        catalog_id: r.catalog_id,
        source: r.source,
        server_id_override: r.server_id_override,
        env_overrides: r.env_overrides,
        trust_grant: r.trust_grant,
        auto_start: r.auto_start,
    }
}

fn map_outcome_to_core(outcome: InstallOutcomeView) -> CoreInstallOutcomeView {
    match outcome {
        InstallOutcomeView::Installed { server_id, started } => CoreInstallOutcomeView {
            kind: "installed".into(),
            server_id: Some(server_id),
            started: Some(started),
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::RuntimeMissing { missing } => CoreInstallOutcomeView {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: missing
                .into_iter()
                .map(|r| r.kind.as_str().into())
                .collect(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::AlreadyInstalled { server_id } => CoreInstallOutcomeView {
            kind: "already_installed".into(),
            server_id: Some(server_id),
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::InvalidEnv { missing_keys } => CoreInstallOutcomeView {
            kind: "invalid_env".into(),
            server_id: None,
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: missing_keys,
        },
    }
}

fn build_server_def(
    entry: &agent_mcp::catalog::ServerEntry,
    req: &agent_mcp::catalog::InstallRequest,
) -> McpServerDef {
    let server_id = req
        .server_id_override
        .clone()
        .unwrap_or_else(|| entry.id.clone());

    let mut env: collections::HashMap<String, String> = entry
        .default_env
        .iter()
        .filter_map(|spec| spec.default.clone().map(|v| (spec.key.clone(), v)))
        .collect();
    for (k, v) in &req.env_overrides {
        env.insert(k.clone(), v.clone());
    }

    let (transport, args) = match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env: spec_env,
            cwd,
        } => {
            for (k, v) in spec_env {
                env.entry(k.clone()).or_insert_with(|| v.clone());
            }
            (
                McpTransportDef::Stdio {
                    command: command.clone(),
                    cwd: cwd.clone(),
                },
                args.clone(),
            )
        }
        InstallSpec::Sse { url, headers } => (
            McpTransportDef::Sse {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
        ),
        InstallSpec::StreamableHttp { url, headers } => (
            McpTransportDef::StreamableHttp {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
        ),
    };

    McpServerDef {
        name: server_id,
        transport,
        args,
        env,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    }
}
