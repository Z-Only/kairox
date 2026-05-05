//! Stdio transport for MCP.
//!
//! Communicates with an MCP server by launching it as a child process and
//! exchanging JSON-RPC messages over its stdin/stdout pipes.

/// Transport that communicates with an MCP server over stdin/stdout.
///
/// TODO: implement in Task 2
pub struct StdioTransport;
