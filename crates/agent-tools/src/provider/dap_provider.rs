use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolProvider};
use agent_lsp::DapClient;
use async_trait::async_trait;
use std::sync::Arc;

pub struct DapToolProvider {
    server_id: String,
    provider_name: String,
    client: Arc<DapClient>,
}

impl DapToolProvider {
    pub fn new(server_id: String, client: Arc<DapClient>) -> Self {
        let provider_name = format!("dap:{server_id}");
        Self {
            server_id,
            provider_name,
            client,
        }
    }

    fn tool_id(&self, op: &str) -> String {
        format!("debug.{}.{}", self.server_id, op)
    }
}

#[async_trait]
impl ToolProvider for DapToolProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                tool_id: self.tool_id("launch"),
                description: "Launch a program under the debugger".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["program"],
                    "properties": {
                        "program": {"type": "string", "description": "Path to program to debug"},
                        "args": {"type": "array", "items": {"type": "string"}, "description": "Program arguments"},
                        "cwd": {"type": "string", "description": "Working directory"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("set_breakpoints"),
                description: "Set breakpoints in a source file".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file", "lines"],
                    "properties": {
                        "file": {"type": "string", "description": "Source file path"},
                        "lines": {"type": "array", "items": {"type": "integer"}, "description": "Line numbers for breakpoints"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("continue"),
                description: "Continue program execution".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thread_id": {"type": "integer", "description": "Thread ID (default: 1)"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("step_over"),
                description: "Step over the current line".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thread_id": {"type": "integer", "description": "Thread ID (default: 1)"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("step_into"),
                description: "Step into the current function call".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thread_id": {"type": "integer", "description": "Thread ID (default: 1)"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("stacktrace"),
                description: "Get the current call stack".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thread_id": {"type": "integer", "description": "Thread ID (default: 1)"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("variables"),
                description: "Inspect variables in a scope".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "frame_id": {"type": "integer", "description": "Stack frame ID"},
                        "scope": {"type": "string", "description": "Scope name (locals, globals, etc.)"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("evaluate"),
                description: "Evaluate an expression in the debugger".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["expression"],
                    "properties": {
                        "expression": {"type": "string", "description": "Expression to evaluate"},
                        "frame_id": {"type": "integer", "description": "Stack frame ID for context"}
                    }
                }),
            },
            ToolDefinition {
                tool_id: self.tool_id("disconnect"),
                description: "End the debug session".into(),
                required_capability: "debug.invoke".into(),
                parameters: serde_json::json!({"type": "object"}),
            },
        ]
    }

    async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
        let prefix = format!("debug.{}.", self.server_id);
        let op = tool_id.strip_prefix(&prefix)?;
        match op {
            "launch" | "set_breakpoints" | "continue" | "step_over" | "step_into"
            | "stacktrace" | "variables" | "evaluate" | "disconnect" => {
                Some(Box::new(DapToolInstance {
                    tool_id: tool_id.to_string(),
                    operation: op.to_string(),
                    client: self.client.clone(),
                }))
            }
            _ => None,
        }
    }
}

struct DapToolInstance {
    tool_id: String,
    operation: String,
    client: Arc<DapClient>,
}

#[async_trait]
impl Tool for DapToolInstance {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: self.tool_id.clone(),
            description: format!("DAP {}", self.operation),
            required_capability: "debug.invoke".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: self.tool_id.clone(),
            effect: ToolEffect::DebugInvoke,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let args = &invocation.arguments;
        let text = match self.operation.as_str() {
            "launch" => {
                let program = get_str(args, "program")?;
                let prog_args: Vec<String> = args
                    .get("args")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let cwd = args.get("cwd").and_then(|v| v.as_str());
                self.client
                    .launch(&program, &prog_args, cwd, None)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                format!("Debug session launched: {program}")
            }
            "set_breakpoints" => {
                let file = get_str(args, "file")?;
                let lines: Vec<u32> = args
                    .get("lines")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let bps = self
                    .client
                    .set_breakpoints(&file, &lines)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                let verified: Vec<_> = bps
                    .iter()
                    .map(|bp| {
                        if bp.verified {
                            format!("  L{}", bp.line.unwrap_or(0))
                        } else {
                            format!("  L{} (unverified)", bp.line.unwrap_or(0))
                        }
                    })
                    .collect();
                format!("Breakpoints in {}:\n{}", file, verified.join("\n"))
            }
            "continue" => {
                let thread_id = get_thread_id(args);
                self.client
                    .continue_execution(thread_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                "Execution continued".to_string()
            }
            "step_over" => {
                let thread_id = get_thread_id(args);
                self.client
                    .step_over(thread_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                "Stepped over".to_string()
            }
            "step_into" => {
                let thread_id = get_thread_id(args);
                self.client
                    .step_into(thread_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                "Stepped into".to_string()
            }
            "stacktrace" => {
                let thread_id = get_thread_id(args);
                let frames = self
                    .client
                    .stack_trace(thread_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                if frames.is_empty() {
                    "No stack frames".to_string()
                } else {
                    frames
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            let src = f
                                .source
                                .as_ref()
                                .and_then(|s| s.path.as_deref())
                                .unwrap_or("??");
                            format!("#{} {} ({}:{})", i, f.name, src, f.line)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            "variables" => {
                let frame_id = args.get("frame_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let scopes = self
                    .client
                    .scopes(frame_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;

                let mut output = Vec::new();
                for scope in &scopes {
                    output.push(format!("--- {} ---", scope.name));
                    let vars = self
                        .client
                        .variables(scope.variables_reference)
                        .await
                        .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                    for var in &vars {
                        let type_str = var
                            .var_type
                            .as_deref()
                            .map(|t| format!(" : {t}"))
                            .unwrap_or_default();
                        output.push(format!("  {}{} = {}", var.name, type_str, var.value));
                    }
                }
                if output.is_empty() {
                    "No variables available".to_string()
                } else {
                    output.join("\n")
                }
            }
            "evaluate" => {
                let expression = get_str(args, "expression")?;
                let frame_id = args.get("frame_id").and_then(|v| v.as_i64());
                let result = self
                    .client
                    .evaluate(&expression, frame_id)
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                result
            }
            "disconnect" => {
                self.client
                    .disconnect()
                    .await
                    .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
                "Debug session disconnected".to_string()
            }
            _ => {
                return Err(crate::ToolError::NotFound(format!(
                    "unknown DAP operation: {}",
                    self.operation
                )));
            }
        };
        Ok(ToolOutput {
            text,
            truncated: false,
        })
    }
}

#[cfg(test)]
#[path = "dap_provider_tests.rs"]
mod tests;

fn get_str(args: &serde_json::Value, key: &str) -> crate::Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            crate::ToolError::ExecutionFailed(format!("missing required parameter: {key}"))
        })
}

fn get_thread_id(args: &serde_json::Value) -> i64 {
    args.get("thread_id").and_then(|v| v.as_i64()).unwrap_or(1)
}
