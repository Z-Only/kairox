/**
 * Browser-side Tauri mock fragment — MCP server settings fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.mcpSettingsServers = [
  {
    id: "github",
    name: "GitHub",
    transport: "stdio",
    enabled: true,
    runtime_status: "running",
    trusted: true,
    tool_count: 6,
    last_error: null,
    writable: true,
    config_path: "/mock/workspace/kairox.toml",
    description: "GitHub MCP server for repository automation."
  },
  {
    id: "builtin-docs",
    name: "Built-in Docs",
    transport: "sse",
    enabled: false,
    runtime_status: "stopped",
    trusted: false,
    tool_count: null,
    last_error: "Disabled by project policy",
    writable: false,
    config_path: null,
    description: "Read-only built-in documentation server."
  }
];
state.disabledMcpServers = [];
