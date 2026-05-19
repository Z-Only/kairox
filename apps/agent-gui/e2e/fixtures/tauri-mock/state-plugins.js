/**
 * Browser-side Tauri mock fragment — plugin settings fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.pluginSettings = [
  {
    settings_id: "Builtin:browser",
    id: "browser",
    name: "Browser",
    description: "Control the in-app browser with Kairox.",
    version: "0.1.0",
    scope: "Builtin",
    path: "builtin://plugins/browser",
    enabled: true,
    install_source: "builtin",
    marketplace: null,
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    inventory: {
      skill_count: 1,
      skill_names: ["browser"],
      mcp_server_count: 0,
      app_count: 0,
      agent_count: 0,
      hook_count: 0
    },
    manifest_kind: "codex"
  },
  {
    settings_id: "User:github",
    id: "github",
    name: "GitHub",
    description: "Inspect repositories, pull requests, issues, and CI.",
    version: "0.1.0",
    scope: "User",
    path: "/Users/mock/.config/kairox/plugins/github",
    enabled: true,
    install_source: "marketplace",
    marketplace: "claude-plugins-official",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    inventory: {
      skill_count: 4,
      skill_names: ["gh-fix-ci", "gh-address-comments"],
      mcp_server_count: 1,
      app_count: 1,
      agent_count: 0,
      hook_count: 0
    },
    manifest_kind: "codex"
  },
  {
    settings_id: "Project:commit-commands",
    id: "commit-commands",
    name: "Commit Commands",
    description: "Git commit and PR automation for this project.",
    version: "1.0.0",
    scope: "Project",
    path: "/mock/workspace/.kairox/plugins/commit-commands",
    enabled: true,
    install_source: "marketplace",
    marketplace: "anthropics-claude-code",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    inventory: {
      skill_count: 2,
      skill_names: ["commit", "pr"],
      mcp_server_count: 0,
      app_count: 0,
      agent_count: 0,
      hook_count: 0
    },
    manifest_kind: "claude"
  }
];
state.pluginMarketplaceSources = [
  {
    id: "claude-plugins-official",
    display_name: "Claude Plugins Official",
    source: "anthropics/claude-plugins-official",
    enabled: true,
    builtin: true
  },
  {
    id: "anthropics-claude-code",
    display_name: "Anthropic Claude Code Demo",
    source: "anthropics/claude-code",
    enabled: true,
    builtin: true
  }
];
state.pluginCatalog = [
  {
    marketplace_id: "claude-plugins-official",
    name: "linear",
    description: "Linear issue and project workflows.",
    version: "0.2.0",
    source: "https://github.com/anthropics/claude-plugins-official/plugins/linear"
  },
  {
    marketplace_id: "anthropics-claude-code",
    name: "quality-review",
    description: "Review code for bugs, security, and performance.",
    version: "1.0.0",
    source: "./plugins/quality-review"
  }
];
