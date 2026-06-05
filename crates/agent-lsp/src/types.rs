use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<JsonRpcErrorObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Status of an LSP or DAP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// DAP protocol message types.
pub mod dap {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize)]
    pub struct DapRequest {
        pub seq: i64,
        #[serde(rename = "type")]
        pub msg_type: String,
        pub command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct DapResponse {
        pub seq: i64,
        #[serde(rename = "type")]
        pub msg_type: String,
        pub request_seq: i64,
        pub command: String,
        pub success: bool,
        #[serde(default)]
        pub message: Option<String>,
        #[serde(default)]
        pub body: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct DapEvent {
        pub seq: i64,
        #[serde(rename = "type")]
        pub msg_type: String,
        pub event: String,
        #[serde(default)]
        pub body: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Breakpoint {
        #[serde(default)]
        pub id: Option<i64>,
        pub verified: bool,
        #[serde(default)]
        pub line: Option<u32>,
        #[serde(default)]
        pub message: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StackFrame {
        pub id: i64,
        pub name: String,
        #[serde(default)]
        pub source: Option<Source>,
        pub line: u32,
        pub column: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Source {
        #[serde(default)]
        pub name: Option<String>,
        #[serde(default)]
        pub path: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Scope {
        pub name: String,
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
        pub expensive: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variable {
        pub name: String,
        pub value: String,
        #[serde(rename = "type", default)]
        pub var_type: Option<String>,
        #[serde(rename = "variablesReference", default)]
        pub variables_reference: Option<i64>,
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
