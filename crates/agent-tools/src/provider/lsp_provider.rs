use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolProvider};
use agent_lsp::LspClient;
use async_trait::async_trait;
use std::sync::Arc;

pub struct LspToolProvider {
    server_id: String,
    provider_name: String,
    client: Arc<LspClient>,
}

impl LspToolProvider {
    pub fn new(server_id: String, client: Arc<LspClient>) -> Self {
        let provider_name = format!("lsp:{server_id}");
        Self {
            server_id,
            provider_name,
            client,
        }
    }

    fn tool_id(&self, op: &str) -> String {
        format!("lsp.{}.{}", self.server_id, op)
    }
}

#[async_trait]
impl ToolProvider for LspToolProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                tool_id: self.tool_id("goto_definition"),
                description: "Go to symbol definition at a file position".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file", "line", "character"],
                    "properties": {
                        "file": {"type": "string", "description": "File path or URI"},
                        "line": {"type": "integer", "description": "0-based line number"},
                        "character": {"type": "integer", "description": "0-based character offset"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("find_references"),
                description: "Find all references to the symbol at a file position".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file", "line", "character"],
                    "properties": {
                        "file": {"type": "string", "description": "File path or URI"},
                        "line": {"type": "integer", "description": "0-based line number"},
                        "character": {"type": "integer", "description": "0-based character offset"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("hover"),
                description: "Get type information and documentation for symbol at position".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file", "line", "character"],
                    "properties": {
                        "file": {"type": "string", "description": "File path or URI"},
                        "line": {"type": "integer", "description": "0-based line number"},
                        "character": {"type": "integer", "description": "0-based character offset"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("document_symbols"),
                description: "List all symbols in a file".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file"],
                    "properties": {
                        "file": {"type": "string", "description": "File path or URI"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("workspace_symbols"),
                description: "Search for symbols across the workspace".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": {"type": "string", "description": "Symbol name or pattern to search"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("diagnostics"),
                description: "Get diagnostics (errors, warnings) for a file".into(),
                required_capability: "lsp.query".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file"],
                    "properties": {
                        "file": {"type": "string", "description": "File path or URI"}
                    }
                }),
            },
        ]
    }

    async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
        let prefix = format!("lsp.{}.", self.server_id);
        let op = tool_id.strip_prefix(&prefix)?;
        match op {
            "goto_definition" | "find_references" | "hover" | "document_symbols"
            | "workspace_symbols" | "diagnostics" => Some(Box::new(LspToolInstance {
                tool_id: tool_id.to_string(),
                operation: op.to_string(),
                client: self.client.clone(),
            })),
            _ => None,
        }
    }
}

struct LspToolInstance {
    tool_id: String,
    operation: String,
    client: Arc<LspClient>,
}

#[async_trait]
impl Tool for LspToolInstance {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: self.tool_id.clone(),
            description: format!("LSP {}", self.operation),
            required_capability: "lsp.query".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: self.tool_id.clone(),
            effect: ToolEffect::LspQuery,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let args = &invocation.arguments;
        let text = match self.operation.as_str() {
            "goto_definition" => {
                let file = get_str(args, "file")?;
                let line = get_u32(args, "line")?;
                let character = get_u32(args, "character")?;
                let locations = self
                    .client
                    .goto_definition(&file, line, character)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                format_locations(&locations)
            }
            "find_references" => {
                let file = get_str(args, "file")?;
                let line = get_u32(args, "line")?;
                let character = get_u32(args, "character")?;
                let locations = self
                    .client
                    .find_references(&file, line, character)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                format_locations(&locations)
            }
            "hover" => {
                let file = get_str(args, "file")?;
                let line = get_u32(args, "line")?;
                let character = get_u32(args, "character")?;
                let hover = self
                    .client
                    .hover(&file, line, character)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                match hover {
                    Some(h) => format_hover(&h),
                    None => "No hover information available".to_string(),
                }
            }
            "document_symbols" => {
                let file = get_str(args, "file")?;
                let symbols = self
                    .client
                    .document_symbols(&file)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                format_document_symbols(&symbols)
            }
            "workspace_symbols" => {
                let query = get_str(args, "query")?;
                let symbols = self
                    .client
                    .workspace_symbols(&query)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                format_workspace_symbols(&symbols)
            }
            "diagnostics" => {
                // LSP diagnostics are pushed via notifications, not request-based.
                // For now, return a message indicating this limitation.
                "Diagnostics are delivered via LSP notifications. Use document_symbols or hover for file-level information.".to_string()
            }
            _ => {
                return Err(crate::ToolError::NotFound(format!(
                    "unknown LSP operation: {}",
                    self.operation
                )));
            }
        };
        Ok(ToolOutput {
            text,
            truncated: false,
            images: vec![],
        })
    }
}

fn get_str(args: &serde_json::Value, key: &str) -> crate::Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            crate::ToolError::ExecutionFailed(format!("missing required parameter: {key}"))
        })
}

fn get_u32(args: &serde_json::Value, key: &str) -> crate::Result<u32> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .ok_or_else(|| {
            crate::ToolError::ExecutionFailed(format!("missing required parameter: {key}"))
        })
}

fn format_locations(locations: &[lsp_types::Location]) -> String {
    if locations.is_empty() {
        return "No locations found".to_string();
    }
    locations
        .iter()
        .map(|loc| {
            format!(
                "{}:{}:{}",
                loc.uri.as_str(),
                loc.range.start.line + 1,
                loc.range.start.character + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_hover(hover: &lsp_types::Hover) -> String {
    match &hover.contents {
        lsp_types::HoverContents::Scalar(ms) => format_markup(ms),
        lsp_types::HoverContents::Array(arr) => arr
            .iter()
            .map(format_markup)
            .collect::<Vec<_>>()
            .join("\n---\n"),
        lsp_types::HoverContents::Markup(mc) => mc.value.clone(),
    }
}

fn format_markup(ms: &lsp_types::MarkedString) -> String {
    match ms {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}

fn format_document_symbols(symbols: &[lsp_types::DocumentSymbol]) -> String {
    if symbols.is_empty() {
        return "No symbols found".to_string();
    }
    let mut lines = Vec::new();
    format_symbols_recursive(symbols, 0, &mut lines);
    lines.join("\n")
}

fn format_symbols_recursive(
    symbols: &[lsp_types::DocumentSymbol],
    depth: usize,
    lines: &mut Vec<String>,
) {
    for sym in symbols {
        let indent = "  ".repeat(depth);
        lines.push(format!(
            "{}{} {:?} [L{}:{}]",
            indent,
            sym.name,
            sym.kind,
            sym.range.start.line + 1,
            sym.range.end.line + 1,
        ));
        if let Some(children) = &sym.children {
            format_symbols_recursive(children, depth + 1, lines);
        }
    }
}

#[allow(deprecated)]
#[cfg(test)]
#[path = "lsp_provider_tests.rs"]
mod tests;

fn format_workspace_symbols(symbols: &[lsp_types::SymbolInformation]) -> String {
    if symbols.is_empty() {
        return "No symbols found".to_string();
    }
    symbols
        .iter()
        .map(|sym| {
            format!(
                "{} {:?} {}:{}",
                sym.name,
                sym.kind,
                sym.location.uri.as_str(),
                sym.location.range.start.line + 1,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
