use crate::catalog_sink::CatalogEventSink;
use crate::facade_runtime::LocalRuntime;
use agent_core::{
    AddCatalogSourceRequest, AgentId, CatalogQuery as CoreCatalogQuery, CatalogSourceView,
    DomainEvent, EventPayload, InstallOutcomeView as CoreInstallOutcomeView,
    InstallRequest as CoreInstallRequest, InstalledEntry as CoreInstalledEntry,
    PrivacyClassification, ServerEntry as CoreServerEntry, SessionId, WorkspaceId,
};
use agent_mcp::catalog::skills::{
    aggregate::AggregateSkillCatalogProvider,
    remote::{build_skill_provider, RemoteSkillSourceConfig, SkillSourceKind},
    SkillCatalogProvider,
};
use agent_mcp::catalog::{
    AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider, CatalogQuery,
    InstallRequest as McpInstallRequest, ServerEntry, TrustLevel,
};
use agent_mcp::installer::{InstallOutcomeView, Installer, OsRuntimeProbe};
use agent_mcp::types::{McpServerDef, McpTransportDef};
use agent_mcp::{
    build_remote_catalog_provider, HttpResponseCache, InstallSpec, RemoteSourceConfig,
    RemoteSourceKind, SharedHttpClient,
};
use agent_store::EventStore;
use std::path::PathBuf;
use std::sync::Arc;

// ── Builder methods ───────────────────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Configure the skill catalog with a cache directory. Creates an internal
    /// HTTP client automatically.
    pub fn with_skill_catalog(mut self, dir: Option<PathBuf>) -> Self {
        if let Some(ref d) = dir {
            self.skill_sources_toml = Some(crate::skill_sources_toml::SkillSourcesToml::new(d));
        }
        self.skill_catalog_http = SharedHttpClient::new().ok();
        self.skill_catalog_cache_dir = dir;
        self
    }

    /// Wire the MCP marketplace: built-in catalog provider + on-disk installer
    /// targeting `<config_dir>/config.toml`.
    ///
    /// Without this, the catalog-related [`AppFacade`] methods return errors
    /// (or empty results) because they have nowhere to read from or write to.
    pub fn with_marketplace(self, config_dir: PathBuf) -> crate::Result<Self> {
        self.with_marketplace_loaded(config_dir, &[])
    }

    /// Phase 2: like [`with_marketplace`] but also registers user-configured
    /// remote catalog sources. The runtime stores the marketplace directory
    /// for future atomic toml mutations + reloads.
    pub fn with_marketplace_loaded(
        mut self,
        config_dir: PathBuf,
        sources: &[agent_config::CatalogSourceConfig],
    ) -> crate::Result<Self> {
        let cache_dir = config_dir.join("catalog-cache");
        let event_tx = self.event_tx.clone();
        let aggregate = build_catalog_provider(sources, cache_dir.clone(), event_tx)
            .map_err(|e| crate::RuntimeError::Other(format!("catalog provider: {e}")))?;
        let aggregate_arc = Arc::new(aggregate);
        let dyn_arc: Arc<dyn CatalogProvider> = aggregate_arc.clone();
        self.aggregate_handle = Some(aggregate_arc);
        self.catalog = Some(dyn_arc);

        let toml_path = config_dir.join("config.toml");
        self.installer = Some(Arc::new(Installer::new(
            toml_path,
            Arc::new(OsRuntimeProbe),
        )));
        self.catalog_http = Some(
            SharedHttpClient::new()
                .map_err(|e| crate::RuntimeError::Other(format!("http client: {e}")))?,
        );
        self.catalog_cache = Some(Arc::new(HttpResponseCache::new(cache_dir)));
        self.marketplace_dir = Some(config_dir);
        Ok(self)
    }

    /// Rebuild the skill catalog aggregate from `skill_sources.toml` and
    /// re-create providers. Called after every toml mutation so the runtime
    /// always reflects the latest persisted configuration.
    pub(crate) fn rebuild_skill_aggregate(&self) -> agent_core::Result<()> {
        let Some(toml) = &self.skill_sources_toml else {
            return Ok(());
        };
        let http = self.skill_catalog_http.clone().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog http not configured".into())
        })?;
        let sources = toml.merge_with_defaults(&toml.read());
        let providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)> = sources
            .into_iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let kind = SkillSourceKind::from_str(&s.kind)?;
                let cfg = RemoteSkillSourceConfig {
                    id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    kind,
                    url: s.url.clone(),
                    search_template: s.search_template.clone(),
                    download_template: s.download_template.clone(),
                    list_template: s.list_template.clone(),
                    detail_template: s.detail_template.clone(),
                    enabled: s.enabled,
                    priority: s.priority,
                    cache_ttl_seconds: s.cache_ttl_seconds,
                };
                Some((s.priority, build_skill_provider(cfg, http.clone())))
            })
            .collect();
        if let Some(catalog) = self.skill_catalog.get() {
            catalog.reload(providers);
        } else {
            let agg = Arc::new(AggregateSkillCatalogProvider::new(providers));
            let _ = self.skill_catalog.set(agg);
        }
        Ok(())
    }

    /// Get (or lazily build) the skill catalog aggregate. Returns `None`
    /// only when the catalog has never been configured.
    pub(crate) fn ensure_skill_catalog(&self) -> Option<Arc<AggregateSkillCatalogProvider>> {
        if let Some(c) = self.skill_catalog.get() {
            return Some(c.clone());
        }
        let _ = self.rebuild_skill_aggregate();
        self.skill_catalog.get().cloned()
    }
}

// ── Marketplace catalog / catalog-source inherent methods ─────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_catalog(
        &self,
        query: CoreCatalogQuery,
    ) -> agent_core::Result<Vec<CoreServerEntry>> {
        let inner_query = map_query(query);
        let entries = match self.catalog.as_ref() {
            Some(catalog) => catalog
                .list(&inner_query)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog list: {e}")))?,
            None => {
                let builtin = builtin_only_provider()?;
                builtin.list(&inner_query).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog list: {e}"))
                })?
            }
        };
        Ok(entries.into_iter().map(map_entry_to_core).collect())
    }

    pub(crate) async fn get_catalog_entry(
        &self,
        id: String,
        _source: Option<String>,
    ) -> agent_core::Result<Option<CoreServerEntry>> {
        let entry =
            match self.catalog.as_ref() {
                Some(catalog) => catalog.get(&id).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                })?,
                None => {
                    let builtin = builtin_only_provider()?;
                    builtin.get(&id).await.map_err(|e| {
                        agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                    })?
                }
            };
        Ok(entry.map(map_entry_to_core))
    }

    pub(crate) async fn refresh_catalog(&self, _source: Option<String>) -> agent_core::Result<()> {
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok(());
        };

        catalog
            .refresh()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog refresh: {e}")))?;
        let entry_count = catalog
            .list(&CatalogQuery::default())
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogRefreshed {
                source: "aggregate".into(),
                entry_count,
            },
        );
        Ok(())
    }

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

    // ── Catalog source mutations ───────────────────────────────────────

    pub(crate) async fn list_catalog_sources(&self) -> agent_core::Result<Vec<CatalogSourceView>> {
        let builtin_view = builtin_source_view();

        let user_sources = match self.marketplace_dir.as_ref() {
            Some(dir) => {
                let mt = crate::marketplace_toml::MarketplaceToml::new(dir);
                mt.read_sources()
                    .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?
            }
            None => Vec::new(),
        };
        let merged = agent_config::merge_with_defaults(user_sources);

        let mut out = Vec::with_capacity(merged.len() + 1);
        out.push(builtin_view);
        for s in merged {
            out.push(catalog_source_to_view(s));
        }
        Ok(out)
    }

    pub(crate) async fn add_catalog_source(
        &self,
        request: AddCatalogSourceRequest,
    ) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let cfg = request_to_source_config(request)?;
        let id = cfg.id.clone();
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.add_source(cfg)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await?;
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogSourceAdded { source: id },
        );
        Ok(())
    }

    pub(crate) async fn remove_catalog_source(&self, id: String) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.remove_source(&id)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    pub(crate) async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.set_enabled(&id, enabled)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    /// Rebuild the aggregate's remote provider list from
    /// `<marketplace_dir>/config.toml`, calling
    /// [`AggregateCatalogProvider::reload`]. The builtin provider is
    /// always re-added at priority 0.
    pub(crate) async fn rebuild_aggregate_from_disk(&self) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let aggregate = self.aggregate_handle.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let http = self.catalog_http.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let cache = self.catalog_cache.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;

        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        let user_sources = mt
            .read_sources()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("config toml: {e}")))?;
        let sources = agent_config::merge_with_defaults(user_sources);

        let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
        let builtin = Arc::new(
            agent_mcp::catalog::BuiltinCatalogProvider::new().map_err(|e| {
                agent_core::CoreError::InvalidState(format!("builtin catalog: {e}"))
            })?,
        );
        providers.push((0, builtin));
        for s in sources.iter().filter(|s| s.enabled) {
            let cfg = agent_mcp::RemoteSourceConfig {
                id: s.id.clone(),
                display_name: s.display_name.clone(),
                kind: match s.kind {
                    agent_config::CatalogSourceKind::McpRegistry => {
                        agent_mcp::RemoteSourceKind::McpRegistry
                    }
                },
                url: s.url.clone(),
                api_key_env: s.api_key_env.clone(),
                priority: s.priority,
                default_trust: parse_trust_str(&s.default_trust),
                enabled: true,
                cache_ttl_seconds: s.cache_ttl_seconds,
            };
            providers.push((
                s.priority,
                agent_mcp::build_remote_catalog_provider(cfg, http.clone(), cache.clone()),
            ));
        }
        aggregate.reload(providers);
        Ok(())
    }
}

// ── Marketplace mapping helpers ───────────────────────────────────────────

pub(crate) fn parse_trust_str(s: &str) -> TrustLevel {
    match s {
        "verified" => TrustLevel::Verified,
        "unverified" => TrustLevel::Unverified,
        _ => TrustLevel::Community,
    }
}

fn map_query(q: CoreCatalogQuery) -> CatalogQuery {
    CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min.as_deref().and_then(parse_trust),
        source: q.source,
        limit: q.limit,
    }
}

fn parse_trust(s: &str) -> Option<TrustLevel> {
    match s {
        "unverified" => Some(TrustLevel::Unverified),
        "community" => Some(TrustLevel::Community),
        "verified" => Some(TrustLevel::Verified),
        _ => None,
    }
}

fn trust_to_str(t: TrustLevel) -> &'static str {
    match t {
        TrustLevel::Unverified => "unverified",
        TrustLevel::Community => "community",
        TrustLevel::Verified => "verified",
    }
}

fn map_entry_to_core(e: ServerEntry) -> CoreServerEntry {
    let install_spec_json = serde_json::to_string(&e.install).unwrap_or_else(|_| "{}".into());
    let requirements_json = serde_json::to_string(&e.requirements).unwrap_or_else(|_| "[]".into());
    let default_env_json = serde_json::to_string(&e.default_env).unwrap_or_else(|_| "[]".into());
    CoreServerEntry {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: trust_to_str(e.trust).into(),
        verified: e.verified,
        icon: e.icon,
        install_spec_json,
        requirements_json,
        default_env_json,
    }
}

fn map_install_request(r: CoreInstallRequest) -> McpInstallRequest {
    McpInstallRequest {
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

fn build_server_def(entry: &ServerEntry, req: &McpInstallRequest) -> McpServerDef {
    let server_id = req
        .server_id_override
        .clone()
        .unwrap_or_else(|| entry.id.clone());

    let mut env: std::collections::HashMap<String, String> = entry
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

fn emit_marketplace_event(tx: &tokio::sync::broadcast::Sender<DomainEvent>, payload: EventPayload) {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    );
    let _ = tx.send(event);
}

fn builtin_source_view() -> CatalogSourceView {
    CatalogSourceView {
        id: "builtin".into(),
        display_name: "Built-in".into(),
        kind: "builtin".into(),
        url: String::new(),
        api_key_env: None,
        priority: 0,
        default_trust: "verified".into(),
        enabled: true,
        cache_ttl_seconds: None,
        last_error: None,
    }
}

/// Build a builtin-only `AggregateCatalogProvider` for the degraded path
/// where the user has no `[mcp_marketplace]` section in `kairox.toml`.
fn builtin_only_provider() -> agent_core::Result<Arc<agent_mcp::catalog::AggregateCatalogProvider>>
{
    static BUILTIN_AGGREGATE: std::sync::OnceLock<
        Arc<agent_mcp::catalog::AggregateCatalogProvider>,
    > = std::sync::OnceLock::new();
    let agg = BUILTIN_AGGREGATE.get_or_init(|| {
        let builtin = Arc::new(
            agent_mcp::catalog::BuiltinCatalogProvider::new()
                .expect("BUILTIN_JSON must parse; this is a build-time invariant"),
        );
        let providers: Vec<Arc<dyn CatalogProvider>> = vec![builtin];
        Arc::new(agent_mcp::catalog::AggregateCatalogProvider::new(providers))
    });
    Ok(Arc::clone(agg))
}

fn catalog_source_to_view(s: agent_config::CatalogSourceConfig) -> CatalogSourceView {
    CatalogSourceView {
        id: s.id,
        display_name: s.display_name,
        kind: match s.kind {
            agent_config::CatalogSourceKind::McpRegistry => "mcp_registry".into(),
        },
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: None,
    }
}

fn request_to_source_config(
    r: AddCatalogSourceRequest,
) -> agent_core::Result<agent_config::CatalogSourceConfig> {
    let kind = match r.kind.as_str() {
        "mcp_registry" => agent_config::CatalogSourceKind::McpRegistry,
        other => {
            return Err(agent_core::CoreError::InvalidState(format!(
                "unsupported catalog source kind: {other}"
            )));
        }
    };
    if !r.url.starts_with("http://") && !r.url.starts_with("https://") {
        return Err(agent_core::CoreError::InvalidState(
            "url must start with http:// or https://".into(),
        ));
    }
    Ok(agent_config::CatalogSourceConfig {
        id: r.id,
        display_name: r.display_name,
        kind,
        url: r.url,
        api_key_env: r.api_key_env,
        priority: r.priority.unwrap_or(100),
        default_trust: r.default_trust.unwrap_or_else(|| "community".into()),
        enabled: r.enabled.unwrap_or(true),
        cache_ttl_seconds: r.cache_ttl_seconds,
    })
}

/// Build the aggregate catalog provider: builtin (priority 0) plus every
/// enabled remote source. Wires a [`CatalogEventSink`] for failure
/// observability.
fn build_catalog_provider(
    sources: &[agent_config::CatalogSourceConfig],
    cache_dir: PathBuf,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
) -> anyhow::Result<AggregateCatalogProvider> {
    let http = SharedHttpClient::new()?;
    let cache = Arc::new(HttpResponseCache::new(cache_dir));

    let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
    let builtin = Arc::new(BuiltinCatalogProvider::new()?);
    providers.push((0, builtin));

    for s in sources.iter().filter(|s| s.enabled) {
        let cfg = RemoteSourceConfig {
            id: s.id.clone(),
            display_name: s.display_name.clone(),
            kind: match s.kind {
                agent_config::CatalogSourceKind::McpRegistry => RemoteSourceKind::McpRegistry,
            },
            url: s.url.clone(),
            api_key_env: s.api_key_env.clone(),
            priority: s.priority,
            default_trust: parse_trust_str(&s.default_trust),
            enabled: true,
            cache_ttl_seconds: s.cache_ttl_seconds,
        };
        let provider = build_remote_catalog_provider(cfg, http.clone(), cache.clone());
        providers.push((s.priority, provider));
    }

    let sink: Arc<dyn agent_mcp::DomainEventSink> = CatalogEventSink::new(event_tx);
    Ok(AggregateCatalogProvider::new_with_priority(
        providers,
        Some(sink),
    ))
}
