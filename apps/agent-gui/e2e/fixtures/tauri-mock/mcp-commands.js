/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- mcp commands ---- */

registerCommandHandlers({
  list_mcp_server_settings: function (args) {
    return clone(state.mcpSettingsServers);
  },
  get_effective_mcp_servers: function (args) {
    return clone(state.mcpSettingsServers.map(effectiveMcpServerView));
  },
  upsert_mcp_server_settings: function (args) {
    var savedServer = createMcpSettingsServer(args.input);
    state.mcpSettingsServers = state.mcpSettingsServers.filter(function (server) {
      return server.id !== savedServer.id;
    });
    state.mcpSettingsServers.push(savedServer);
    return clone(savedServer);
  },
  set_mcp_server_enabled: function (args) {
    var serverToToggle = findMcpSettingsServer(args.serverId);
    if (!serverToToggle) return Promise.reject(new Error("MCP server not found: " + args.serverId));
    serverToToggle.enabled = args.enabled;
    serverToToggle.runtime_status = args.enabled ? "running" : "stopped";
    return null;
  },
  delete_mcp_server_settings: function (args) {
    state.mcpSettingsServers = state.mcpSettingsServers.filter(function (server) {
      return server.id !== args.serverId;
    });
    return null;
  },
  disable_mcp_server_at_scope: function (args) {
    var serverToDisable = findMcpSettingsServer(args.serverId);
    if (!serverToDisable)
      return Promise.reject(new Error("MCP server not found: " + args.serverId));
    if (state.disabledMcpServers.indexOf(args.serverId) < 0) {
      state.disabledMcpServers.push(args.serverId);
      state.disabledMcpServers.sort();
    }
    return null;
  },
  enable_mcp_server_at_scope: function (args) {
    state.disabledMcpServers = state.disabledMcpServers.filter(function (serverId) {
      return serverId !== args.serverId;
    });
    return null;
  },
  refresh_config_for_project: function (args) {
    return null;
  },
  open_mcp_config_file: function (args) {
    if (window.__MCP_OPEN_CONFIG_SHOULD_FAIL__) {
      return Promise.reject(new Error("mock failure"));
    }
    return "/mock/workspace";
  },
  list_mcp_servers: function (args) {
    return state.mcpSettingsServers.map(function (server) {
      return {
        id: server.id,
        status: server.runtime_status,
        tool_count: server.tool_count
      };
    });
  },
  start_mcp_server: function (args) {
    var serverToStart = findMcpSettingsServer(args.serverId);
    if (serverToStart) serverToStart.runtime_status = "running";
    return null;
  },
  stop_mcp_server: function (args) {
    var serverToStop = findMcpSettingsServer(args.serverId);
    if (serverToStop) serverToStop.runtime_status = "stopped";
    return null;
  },
  trust_mcp_server: function (args) {
    var serverToTrust = findMcpSettingsServer(args.serverId);
    if (serverToTrust) serverToTrust.trusted = true;
    return null;
  },
  revoke_mcp_trust: function (args) {
    var serverToRevoke = findMcpSettingsServer(args.serverId);
    if (serverToRevoke) serverToRevoke.trusted = false;
    return null;
  },
  refresh_mcp_tools: function (args) {
    var serverToRefresh = findMcpSettingsServer(args.serverId);
    if (serverToRefresh) serverToRefresh.tool_count = (serverToRefresh.tool_count || 0) + 1;
    return [{ name: "echo", description: "Echo tool", input_schema: null }];
  },
  check_mcp_health: function (args) {
    var serverToCheck = findMcpSettingsServer(args.serverId);
    if (!serverToCheck) return Promise.reject(new Error("MCP server not found: " + args.serverId));
    return {
      tools: [{ name: "echo", description: "Echo tool", input_schema: null }],
      healthy: true,
      error: null
    };
  },
  get_mcp_tool_states: function (args) {
    return { disabled_tools: [] };
  },
  set_mcp_tool_disabled: function (args) {
    return null;
  },
  test_mcp_connectivity: function (args) {
    var serverToTest = findMcpSettingsServer(args.serverId);
    if (!serverToTest) return Promise.reject(new Error("MCP server not found: " + args.serverId));
    return { status: "connected", tool_count: serverToTest.tool_count || 1 };
  },
  list_mcp_resources: function (args) {
    var serverToFetch = findMcpSettingsServer(args.serverId);
    if (!serverToFetch) return Promise.reject(new Error("MCP server not found: " + args.serverId));
    if (serverToFetch.id === "github") {
      return [
        {
          uri: "file://logs/app.log",
          name: "App Log",
          description: "Application log file",
          mime_type: "text/plain"
        },
        {
          uri: "file://config/settings.json",
          name: "Settings",
          description: "Configuration file",
          mime_type: "application/json"
        }
      ];
    }
    return [];
  },
  list_mcp_prompts: function (args) {
    var serverToFetch = findMcpSettingsServer(args.serverId);
    if (!serverToFetch) return Promise.reject(new Error("MCP server not found: " + args.serverId));
    if (serverToFetch.id === "github") {
      return [
        {
          name: "analyze_code",
          description: "Analyze code for bugs and style issues",
          argument_count: 2
        },
        {
          name: "summarize_text",
          description: "Summarize input text to key points",
          argument_count: 1
        },
        { name: "format_output", description: "Format output as JSON or YAML", argument_count: 0 }
      ];
    }
    return [];
  },
  read_mcp_resource: function (args) {
    return [
      {
        type: "text",
        text: "[2026-05-17 10:30:00] INFO Server started\n[2026-05-17 10:30:01] INFO Listening on port 8080"
      }
    ];
  }
});
