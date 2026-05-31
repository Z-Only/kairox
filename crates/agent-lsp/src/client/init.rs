use lsp_types::{
    ClientCapabilities, GeneralClientCapabilities, InitializeParams, ServerCapabilities,
    TextDocumentClientCapabilities, WindowClientCapabilities, WorkspaceClientCapabilities,
};

use crate::error::{LspError, Result};

use super::LspClient;

impl LspClient {
    /// Perform the LSP initialize handshake.
    ///
    /// Sends `initialize`, caches `ServerCapabilities`, then sends `initialized` notification.
    pub async fn initialize(&self, root_uri: &str) -> Result<&ServerCapabilities> {
        self.capabilities
            .get_or_try_init(|| async {
                let root_uri: lsp_types::Uri = root_uri
                    .parse()
                    .or_else(|_| format!("file://{root_uri}").parse())
                    .map_err(|e| LspError::Init(format!("invalid root_uri: {e}")))?;

                #[allow(deprecated)]
                let params = serde_json::to_value(InitializeParams {
                    process_id: Some(std::process::id()),
                    root_uri: Some(root_uri),
                    capabilities: client_capabilities(),
                    initialization_options: None,
                    ..Default::default()
                })
                .map_err(|e| LspError::Init(format!("failed to serialize params: {e}")))?;

                let result = self
                    .send_request("initialize", Some(params))
                    .await?
                    .ok_or_else(|| LspError::Init("initialize returned null result".into()))?;

                let init_result: lsp_types::InitializeResult = serde_json::from_value(result)
                    .map_err(|e| LspError::Init(format!("invalid InitializeResult: {e}")))?;

                // Send initialized notification.
                self.send_notification(
                    "initialized",
                    Some(serde_json::to_value(lsp_types::InitializedParams {}).unwrap()),
                )
                .await?;

                Ok(init_result.capabilities)
            })
            .await
    }
}

fn client_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            ..Default::default()
        }),
        window: Some(WindowClientCapabilities {
            ..Default::default()
        }),
        general: Some(GeneralClientCapabilities {
            ..Default::default()
        }),
        ..Default::default()
    }
}
