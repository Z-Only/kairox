//! LSP/DAP Server Manager — orchestrates LSP and DAP server lifecycle, tool registration, and events.

use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_lsp::{DapServerDef, DapServerLifecycle, LspServerDef, LspServerLifecycle};
use agent_tools::permission::PermissionEngine;
use agent_tools::provider::{DapToolProvider, LspToolProvider};
use agent_tools::registry::ToolRegistry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

pub struct LspServerManager {
    lsp_servers: HashMap<String, LspServerLifecycle>,
    dap_servers: HashMap<String, DapServerLifecycle>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    #[allow(dead_code)]
    permission_engine: Arc<Mutex<PermissionEngine>>,
    event_tx: Option<tokio::sync::broadcast::Sender<DomainEvent>>,
}

pub(crate) fn file_uri_from_path(path: &str) -> String {
    Url::from_file_path(Path::new(path))
        .map(|url| url.to_string())
        .unwrap_or_else(|_| format!("file://{path}"))
}

impl LspServerManager {
    pub fn from_config(
        lsp_configs: Vec<LspServerDef>,
        dap_configs: Vec<DapServerDef>,
        tool_registry: Arc<Mutex<ToolRegistry>>,
        permission_engine: Arc<Mutex<PermissionEngine>>,
        event_tx: Option<tokio::sync::broadcast::Sender<DomainEvent>>,
    ) -> Self {
        let lsp_servers = lsp_configs
            .into_iter()
            .map(|def| (def.name.clone(), LspServerLifecycle::new(def)))
            .collect();
        let dap_servers = dap_configs
            .into_iter()
            .map(|def| (def.name.clone(), DapServerLifecycle::new(def)))
            .collect();
        Self {
            lsp_servers,
            dap_servers,
            tool_registry,
            permission_engine,
            event_tx,
        }
    }

    pub async fn start_lsp_server(
        &mut self,
        server_id: &str,
        root_uri: &str,
    ) -> Result<(), agent_lsp::LspError> {
        let languages = {
            let lifecycle = self.lsp_servers.get(server_id).ok_or_else(|| {
                agent_lsp::LspError::Init(format!("LSP server '{server_id}' not found"))
            })?;
            lifecycle.def.languages.clone()
        };

        self.emit_event(EventPayload::LspServerStarting {
            server_id: server_id.to_string(),
            languages: languages.clone(),
        });

        let client = {
            let lifecycle = self.lsp_servers.get_mut(server_id).unwrap();
            match lifecycle.start(root_uri).await {
                Ok(c) => c,
                Err(e) => {
                    self.emit_event(EventPayload::LspServerFailed {
                        server_id: server_id.to_string(),
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        };

        let provider = LspToolProvider::new(server_id.to_string(), client);
        let mut registry = self.tool_registry.lock().await;
        registry.add_provider(Box::new(provider)).await;

        self.emit_event(EventPayload::LspServerReady {
            server_id: server_id.to_string(),
            languages,
        });

        Ok(())
    }

    pub async fn start_dap_server(&mut self, server_id: &str) -> Result<(), agent_lsp::LspError> {
        if !self.dap_servers.contains_key(server_id) {
            return Err(agent_lsp::LspError::Init(format!(
                "DAP server '{server_id}' not found"
            )));
        }

        let client = {
            let lifecycle = self.dap_servers.get_mut(server_id).unwrap();
            match lifecycle.start().await {
                Ok(c) => c,
                Err(e) => {
                    self.emit_event(EventPayload::LspServerFailed {
                        server_id: server_id.to_string(),
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        };

        let provider = DapToolProvider::new(server_id.to_string(), client);
        let mut registry = self.tool_registry.lock().await;
        registry.add_provider(Box::new(provider)).await;

        Ok(())
    }

    pub async fn start_auto_lsp_servers(
        &mut self,
        root_uri: &str,
    ) -> Vec<Result<(), agent_lsp::LspError>> {
        let ids: Vec<String> = self.lsp_servers.keys().cloned().collect();
        let mut results = Vec::new();
        for id in ids {
            let result = self.start_lsp_server(&id, root_uri).await;
            if let Err(ref e) = result {
                tracing::error!("Failed to start LSP server '{}': {}", id, e);
            }
            results.push(result);
        }
        let dap_ids: Vec<String> = self.dap_servers.keys().cloned().collect();
        for id in dap_ids {
            let result = self.start_dap_server(&id).await;
            if let Err(ref e) = result {
                tracing::error!("Failed to start DAP server '{}': {}", id, e);
            }
            results.push(result);
        }
        results
    }

    pub async fn stop_lsp_server(&mut self, server_id: &str) -> Result<(), agent_lsp::LspError> {
        let lifecycle = self.lsp_servers.get_mut(server_id).ok_or_else(|| {
            agent_lsp::LspError::Init(format!("LSP server '{server_id}' not found"))
        })?;
        lifecycle.stop().await?;
        self.emit_event(EventPayload::LspServerStopped {
            server_id: server_id.to_string(),
        });
        Ok(())
    }

    pub async fn stop_dap_server(&mut self, server_id: &str) -> Result<(), agent_lsp::LspError> {
        let lifecycle = self.dap_servers.get_mut(server_id).ok_or_else(|| {
            agent_lsp::LspError::Init(format!("DAP server '{server_id}' not found"))
        })?;
        lifecycle.stop().await?;
        self.emit_event(EventPayload::DapSessionStopped {
            server_id: server_id.to_string(),
        });
        Ok(())
    }

    pub async fn shutdown_all(&mut self) -> Result<(), agent_lsp::LspError> {
        let lsp_ids: Vec<String> = self.lsp_servers.keys().cloned().collect();
        for id in lsp_ids {
            if let Some(lifecycle) = self.lsp_servers.get_mut(&id) {
                let _ = lifecycle.stop().await;
            }
            self.emit_event(EventPayload::LspServerStopped { server_id: id });
        }
        let dap_ids: Vec<String> = self.dap_servers.keys().cloned().collect();
        for id in dap_ids {
            if let Some(lifecycle) = self.dap_servers.get_mut(&id) {
                let _ = lifecycle.stop().await;
            }
            self.emit_event(EventPayload::DapSessionStopped { server_id: id });
        }
        Ok(())
    }

    pub fn lsp_server_ids(&self) -> Vec<String> {
        self.lsp_servers.keys().cloned().collect()
    }

    pub fn dap_server_ids(&self) -> Vec<String> {
        self.dap_servers.keys().cloned().collect()
    }

    fn emit_event(&self, payload: EventPayload) {
        if let Some(tx) = &self.event_tx {
            let event = DomainEvent::new(
                WorkspaceId::new(),
                SessionId::new(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                payload,
            );
            let _ = tx.send(event);
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
