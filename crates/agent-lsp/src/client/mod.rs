mod init;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use lsp_types::ServerCapabilities;
use tokio::sync::{Mutex, OnceCell};
use url::Url;

use crate::error::{LspError, Result};
use crate::transport::Transport;
use crate::types::{JsonRpcNotification, JsonRpcRequest};

/// LSP client wrapping a transport with request ID management and capability caching.
pub struct LspClient {
    server_id: String,
    transport: Arc<Mutex<Box<dyn Transport>>>,
    capabilities: OnceCell<ServerCapabilities>,
    next_id: AtomicU64,
}

impl LspClient {
    pub fn new(server_id: String, transport: Box<dyn Transport>) -> Self {
        Self {
            server_id,
            transport: Arc::new(Mutex::new(transport)),
            capabilities: OnceCell::new(),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    pub fn capabilities(&self) -> Option<&ServerCapabilities> {
        self.capabilities.get()
    }

    fn next_request_id(&self) -> serde_json::Value {
        serde_json::Value::Number(self.next_id.fetch_add(1, Ordering::SeqCst).into())
    }

    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.next_request_id(),
            method: method.to_string(),
            params,
        };
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        Ok(response.result)
    }

    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        let mut transport = self.transport.lock().await;
        transport.send_notification(notification).await
    }

    pub async fn goto_definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<lsp_types::Location>> {
        let params = serde_json::to_value(lsp_types::GotoDefinitionParams {
            text_document_position_params: text_document_position(uri, line, character),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })?;
        let result = self
            .send_request("textDocument/definition", Some(params))
            .await?;
        match result {
            None => Ok(vec![]),
            Some(value) => parse_location_response(value),
        }
    }

    pub async fn find_references(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<lsp_types::Location>> {
        let params = serde_json::to_value(lsp_types::ReferenceParams {
            text_document_position: text_document_position(uri, line, character),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        })?;
        let result = self
            .send_request("textDocument/references", Some(params))
            .await?;
        match result {
            None => Ok(vec![]),
            Some(value) => {
                let locs: Vec<lsp_types::Location> = serde_json::from_value(value)
                    .map_err(|e| LspError::Protocol(format!("invalid references response: {e}")))?;
                Ok(locs)
            }
        }
    }

    pub async fn hover(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Option<lsp_types::Hover>> {
        let params = serde_json::to_value(lsp_types::HoverParams {
            text_document_position_params: text_document_position(uri, line, character),
            work_done_progress_params: Default::default(),
        })?;
        let result = self
            .send_request("textDocument/hover", Some(params))
            .await?;
        match result {
            None => Ok(None),
            Some(value) => {
                let hover: lsp_types::Hover = serde_json::from_value(value)
                    .map_err(|e| LspError::Protocol(format!("invalid hover response: {e}")))?;
                Ok(Some(hover))
            }
        }
    }

    pub async fn document_symbols(&self, uri: &str) -> Result<Vec<lsp_types::DocumentSymbol>> {
        let params = serde_json::to_value(lsp_types::DocumentSymbolParams {
            text_document: text_document_identifier(uri),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })?;
        let result = self
            .send_request("textDocument/documentSymbol", Some(params))
            .await?;
        match result {
            None => Ok(vec![]),
            Some(value) => {
                // Response can be DocumentSymbol[] or SymbolInformation[].
                if let Ok(symbols) =
                    serde_json::from_value::<Vec<lsp_types::DocumentSymbol>>(value.clone())
                {
                    return Ok(symbols);
                }
                if let Ok(infos) =
                    serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(value)
                {
                    #[allow(deprecated)]
                    return Ok(infos
                        .into_iter()
                        .map(|info| lsp_types::DocumentSymbol {
                            name: info.name,
                            kind: info.kind,
                            detail: None,
                            tags: info.tags,
                            deprecated: info.deprecated,
                            range: info.location.range,
                            selection_range: info.location.range,
                            children: None,
                        })
                        .collect());
                }
                Ok(vec![])
            }
        }
    }

    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<Vec<lsp_types::SymbolInformation>> {
        let params = serde_json::to_value(lsp_types::WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })?;
        let result = self.send_request("workspace/symbol", Some(params)).await?;
        match result {
            None => Ok(vec![]),
            Some(value) => {
                let symbols: Vec<lsp_types::SymbolInformation> = serde_json::from_value(value)
                    .map_err(|e| {
                        LspError::Protocol(format!("invalid workspace/symbol response: {e}"))
                    })?;
                Ok(symbols)
            }
        }
    }

    pub async fn completion(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<lsp_types::CompletionItem>> {
        let params = serde_json::to_value(lsp_types::CompletionParams {
            text_document_position: text_document_position(uri, line, character),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        })?;
        let result = self
            .send_request("textDocument/completion", Some(params))
            .await?;
        match result {
            None => Ok(vec![]),
            Some(value) => {
                // Response can be CompletionItem[] or CompletionList.
                if let Ok(items) =
                    serde_json::from_value::<Vec<lsp_types::CompletionItem>>(value.clone())
                {
                    return Ok(items);
                }
                if let Ok(list) = serde_json::from_value::<lsp_types::CompletionList>(value) {
                    return Ok(list.items);
                }
                Ok(vec![])
            }
        }
    }

    pub async fn did_open(&self, uri: &str, language_id: &str, text: &str) -> Result<()> {
        let params = serde_json::to_value(lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: parse_uri(uri)?,
                language_id: language_id.to_string(),
                version: 0,
                text: text.to_string(),
            },
        })?;
        self.send_notification("textDocument/didOpen", Some(params))
            .await
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.send_request("shutdown", None).await?;
        self.send_notification("exit", None).await?;
        let mut transport = self.transport.lock().await;
        transport.close().await
    }
}

fn text_document_position(
    uri: &str,
    line: u32,
    character: u32,
) -> lsp_types::TextDocumentPositionParams {
    lsp_types::TextDocumentPositionParams {
        text_document: text_document_identifier(uri),
        position: lsp_types::Position { line, character },
    }
}

fn text_document_identifier(uri: &str) -> lsp_types::TextDocumentIdentifier {
    lsp_types::TextDocumentIdentifier {
        uri: parse_uri_infallible(uri),
    }
}

fn parse_uri_infallible(uri: &str) -> lsp_types::Uri {
    parse_uri(uri).unwrap_or_else(|_| {
        format!("file://{uri}")
            .parse::<lsp_types::Uri>()
            .expect("fallback URI parse")
    })
}

fn parse_uri(uri: &str) -> Result<lsp_types::Uri> {
    normalized_uri(uri)
        .parse::<lsp_types::Uri>()
        .or_else(|_| format!("file://{uri}").parse::<lsp_types::Uri>())
        .map_err(|e| LspError::Protocol(format!("invalid URI '{uri}': {e}")))
}

fn normalized_uri(uri_or_path: &str) -> String {
    let path = Path::new(uri_or_path);
    if path.is_absolute() {
        if let Ok(url) = Url::from_file_path(path) {
            return url.to_string();
        }
    }

    uri_or_path.to_string()
}

fn parse_location_response(value: serde_json::Value) -> Result<Vec<lsp_types::Location>> {
    // Can be Location | Location[] | LocationLink[]
    if let Ok(loc) = serde_json::from_value::<lsp_types::Location>(value.clone()) {
        return Ok(vec![loc]);
    }
    if let Ok(locs) = serde_json::from_value::<Vec<lsp_types::Location>>(value.clone()) {
        return Ok(locs);
    }
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(value) {
        return Ok(links
            .into_iter()
            .map(|link| lsp_types::Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect());
    }
    Ok(vec![])
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
