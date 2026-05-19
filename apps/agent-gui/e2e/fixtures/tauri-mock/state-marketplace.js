/**
 * Browser-side Tauri mock fragment — marketplace fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.catalog = [
  {
    id: "filesystem",
    source: "builtin",
    display_name: "Filesystem",
    summary: "Read, write, and search files inside an allow-listed directory.",
    description: "Provides safe filesystem access scoped to a workspace path.",
    categories: ["filesystem", "dev-tools"],
    tags: ["files", "fs"],
    author: "MCP",
    homepage: "https://github.com/modelcontextprotocol/servers",
    version: "0.6.0",
    trust: "verified",
    icon: "📁",
    install_spec_json: JSON.stringify({
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "${WORKSPACE_PATH}"],
      env: {},
      cwd: null
    }),
    requirements_json: JSON.stringify([
      {
        kind: "node",
        min_version: ">=18.0.0",
        install_hint: "https://nodejs.org"
      }
    ]),
    default_env_json: JSON.stringify([
      {
        key: "WORKSPACE_PATH",
        label: "Workspace path",
        description: "Directory the server can read",
        required: true,
        secret: false,
        default: "/tmp"
      }
    ])
  }
];
state.installedCatalog = [];
state.catalogRuntimePresent = { node: true, python: true, uvx: true, docker: true };
// Phase 2: catalog sources — only user-configured remote sources are listed here.
// The builtin source is implicit (the GUI's source chip bar always renders a
// "Built-in" chip in addition to whatever list_catalog_sources returns).
state.catalogSources = [];
