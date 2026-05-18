/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- agent settings commands ---- */

registerCommandHandlers({
  list_agent_settings: function (args) {
    refreshAgentSettingsEffectiveness();
    return clone(state.agentSettings);
  },
  upsert_agent_settings: function (args) {
    var nextAgent = createAgentSetting(args.input || {});
    state.agentSettings = state.agentSettings.filter(function (agent) {
      return agent.settingsId !== nextAgent.settingsId;
    });
    state.agentSettings.push(nextAgent);
    refreshAgentSettingsEffectiveness();
    return clone(nextAgent);
  },
  delete_agent_settings: function (args) {
    var agentToDelete = findAgentSetting(args.agentId);
    if (!agentToDelete)
      return Promise.reject(new Error("Agent setting not found: " + args.agentId));
    if (!agentToDelete.deletable)
      return Promise.reject(new Error("Agent setting is not deletable: " + args.agentId));
    state.agentSettings = state.agentSettings.filter(function (agent) {
      return agent.settingsId !== agentToDelete.settingsId;
    });
    refreshAgentSettingsEffectiveness();
    return null;
  },
  copy_agent_settings: function (args) {
    var sourceAgent = findAgentSetting(args.agentId);
    if (!sourceAgent) return Promise.reject(new Error("Agent setting not found: " + args.agentId));
    var targetScope = args.scope || "User";
    var copiedAgent = Object.assign({}, sourceAgent, {
      settingsId: targetScope + ":" + sourceAgent.name,
      scope: targetScope,
      path: agentSettingsPath(targetScope, sourceAgent.name),
      editable: targetScope !== "Builtin",
      deletable: targetScope !== "Builtin"
    });
    state.agentSettings = state.agentSettings.filter(function (agent) {
      return agent.settingsId !== copiedAgent.settingsId;
    });
    state.agentSettings.push(copiedAgent);
    refreshAgentSettingsEffectiveness();
    return clone(copiedAgent);
  },
  open_agents_dir: function (args) {
    return "/Users/mock/.config/kairox/agents";
  }
});
