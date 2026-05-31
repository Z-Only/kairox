use std::collections::HashMap;
use std::sync::Arc;

use crate::client::LspClient;
use crate::dap::DapClient;
use crate::error::Result;
use crate::transport::stdio::LspStdioTransport;
use crate::types::ServerStatus;

/// Definition for an LSP or DAP server, parsed from config.
#[derive(Debug, Clone)]
pub struct LspServerDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
    pub languages: Vec<String>,
    pub file_patterns: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct DapServerDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
    pub languages: Vec<String>,
}

/// Manages the lifecycle of a single LSP server.
pub struct LspServerLifecycle {
    pub def: LspServerDef,
    client: Option<Arc<LspClient>>,
    status: ServerStatus,
    root_uri: Option<String>,
}

impl LspServerLifecycle {
    pub fn new(def: LspServerDef) -> Self {
        Self {
            def,
            client: None,
            status: ServerStatus::Stopped,
            root_uri: None,
        }
    }

    pub fn status(&self) -> &ServerStatus {
        &self.status
    }

    pub fn client(&self) -> Option<&Arc<LspClient>> {
        self.client.as_ref()
    }

    pub async fn start(&mut self, root_uri: &str) -> Result<Arc<LspClient>> {
        if let Some(client) = &self.client {
            if self.status == ServerStatus::Running && self.root_uri.as_deref() == Some(root_uri) {
                return Ok(client.clone());
            }
            let _ = client.shutdown().await;
            self.client = None;
            self.root_uri = None;
        }
        self.status = ServerStatus::Starting;

        let args_refs: Vec<&str> = self.def.args.iter().map(|s| s.as_str()).collect();
        let transport = LspStdioTransport::spawn(
            &self.def.command,
            &args_refs,
            self.def.env.clone(),
            self.def.cwd.as_deref(),
        )
        .await
        .inspect_err(|e| {
            self.status = ServerStatus::Error(e.to_string());
        })?;

        let client = Arc::new(LspClient::new(self.def.name.clone(), Box::new(transport)));
        client.initialize(root_uri).await.inspect_err(|e| {
            self.status = ServerStatus::Error(e.to_string());
        })?;

        self.client = Some(client.clone());
        self.root_uri = Some(root_uri.to_string());
        self.status = ServerStatus::Running;
        tracing::info!(server = %self.def.name, "LSP server started");
        Ok(client)
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            let _ = client.shutdown().await;
        }
        self.root_uri = None;
        self.status = ServerStatus::Stopped;
        tracing::info!(server = %self.def.name, "LSP server stopped");
        Ok(())
    }
}

/// Manages the lifecycle of a single DAP server.
pub struct DapServerLifecycle {
    pub def: DapServerDef,
    client: Option<Arc<DapClient>>,
    status: ServerStatus,
}

impl DapServerLifecycle {
    pub fn new(def: DapServerDef) -> Self {
        Self {
            def,
            client: None,
            status: ServerStatus::Stopped,
        }
    }

    pub fn status(&self) -> &ServerStatus {
        &self.status
    }

    pub fn client(&self) -> Option<&Arc<DapClient>> {
        self.client.as_ref()
    }

    pub async fn start(&mut self) -> Result<Arc<DapClient>> {
        if let Some(client) = &self.client {
            if self.status == ServerStatus::Running {
                return Ok(client.clone());
            }
        }
        self.status = ServerStatus::Starting;

        let args_refs: Vec<&str> = self.def.args.iter().map(|s| s.as_str()).collect();
        let transport = LspStdioTransport::spawn(
            &self.def.command,
            &args_refs,
            self.def.env.clone(),
            self.def.cwd.as_deref(),
        )
        .await
        .inspect_err(|e| {
            self.status = ServerStatus::Error(e.to_string());
        })?;

        let client = Arc::new(DapClient::new(self.def.name.clone(), Box::new(transport)));
        client.initialize().await.inspect_err(|e| {
            self.status = ServerStatus::Error(e.to_string());
        })?;

        self.client = Some(client.clone());
        self.status = ServerStatus::Running;
        tracing::info!(server = %self.def.name, "DAP server started");
        Ok(client)
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            let _ = client.disconnect().await;
        }
        self.status = ServerStatus::Stopped;
        tracing::info!(server = %self.def.name, "DAP server stopped");
        Ok(())
    }
}
