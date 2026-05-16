/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- Helpers ---- */

function getSession(sessionId) {
  const id = sessionId || state.currentSessionId;
  return state.sessions.find(function (s) {
    return s.id === id;
  });
}

function getProject(projectId) {
  return state.projects.find(function (project) {
    return project.project_id === projectId;
  });
}

function makeSessionInfo(id, title, profile, projectId, worktreePath, branch, visibility) {
  return {
    id: id,
    title: title,
    profile: profile,
    project_id: projectId || null,
    worktree_path: worktreePath || null,
    branch: branch || null,
    visibility: visibility || "visible"
  };
}

function makeProjectGitStatus(project) {
  return {
    kind: state.gitStatuses.get(project.project_id) || "not_initialized",
    branch: null,
    worktree_path: project.root_path,
    message: null
  };
}

function getProjectSessionList(projectId) {
  return state.projectSessions.get(projectId) || [];
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function snapshotMap(map) {
  return Array.from(map.entries()).map(function (entry) {
    return [entry[0], clone(entry[1])];
  });
}

function persistMockState() {
  try {
    localStorage.setItem(
      persistedStateKey,
      JSON.stringify({
        idCounter: idCounter,
        initialized: state.initialized,
        workspace: state.workspace,
        sessions: state.sessions,
        projects: state.projects,
        projectSessions: snapshotMap(state.projectSessions),
        archivedSessions: state.archivedSessions,
        gitStatuses: snapshotMap(state.gitStatuses),
        currentSessionId: state.currentSessionId,
        currentProfile: state.currentProfile,
        projections: snapshotMap(state.projections),
        traces: snapshotMap(state.traces),
        drafts: snapshotMap(state.drafts)
      })
    );
  } catch {
    // The mock can be evaluated in non-origin contexts where localStorage is unavailable.
  }
}

function restorePersistedMockState() {
  try {
    var raw = localStorage.getItem(persistedStateKey);
    if (!raw) return;
    var snapshot = JSON.parse(raw);
    idCounter = snapshot.idCounter || 0;
    state.initialized = Boolean(snapshot.initialized);
    state.workspace = snapshot.workspace || null;
    state.sessions = snapshot.sessions || [];
    state.projects = snapshot.projects || [];
    state.projectSessions = new Map(snapshot.projectSessions || []);
    state.archivedSessions = snapshot.archivedSessions || [];
    state.gitStatuses = new Map(snapshot.gitStatuses || []);
    state.currentSessionId = snapshot.currentSessionId || null;
    state.currentProfile = snapshot.currentProfile || "fast";
    state.projections = new Map(snapshot.projections || []);
    state.traces = new Map(snapshot.traces || []);
    state.drafts = new Map(snapshot.drafts || []);
  } catch {
    try {
      localStorage.removeItem(persistedStateKey);
    } catch {}
  }
}

function slugify(value) {
  return String(value)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function findMcpSettingsServer(serverId) {
  return state.mcpSettingsServers.find(function (server) {
    return server.id === serverId;
  });
}

function findSkillSetting(skillId) {
  var settingsIdMatches = state.skillSettings.filter(function (skill) {
    return skill.settings_id === skillId;
  });
  if (settingsIdMatches.length === 1) {
    return settingsIdMatches[0];
  }
  if (settingsIdMatches.length > 1) {
    throw new Error("ambiguous skill settings id: " + skillId);
  }

  var legacyIdMatches = state.skillSettings.filter(function (skill) {
    return skill.id === skillId;
  });
  if (legacyIdMatches.length === 1) {
    return legacyIdMatches[0];
  }
  if (legacyIdMatches.length > 1) {
    throw new Error("ambiguous skill id: " + skillId);
  }

  return null;
}

function createMcpSettingsServer(input) {
  var serverId = slugify(input.name || "mcp-server");
  var transport =
    input.transport && input.transport.transport ? input.transport.transport : "stdio";
  return {
    id: serverId,
    name: input.name,
    transport: transport,
    enabled: input.enabled,
    runtime_status: input.enabled ? "running" : "stopped",
    trusted: false,
    tool_count: transport === "stdio" ? 1 : null,
    last_error: null,
    writable: true,
    config_path: "/mock/workspace/kairox.toml",
    description: input.description || null
  };
}

function createSkillSettingFromInstall(name, source, target, installSource) {
  var skillId = slugify(name);
  return {
    settings_id: target + ":" + skillId,
    id: skillId,
    name: name,
    description: "Installed from " + source + ".",
    version: "0.1.0",
    scope: target,
    path:
      target === "user"
        ? "/Users/mock/.kairox/skills/" + skillId + "/SKILL.md"
        : "/mock/workspace/.kairox/skills/" + skillId + "/SKILL.md",
    enabled: true,
    activation_mode: "manual",
    install_source: installSource,
    update_state: "up_to_date",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    editable: true,
    deletable: true
  };
}

function configScopeFromSource(source) {
  switch (source) {
    case "builtin":
    case "defaults":
      return "Builtin";
    case "project":
    case "project_config":
      return "Project";
    case "local":
      return "Local";
    case "user":
    case "user_config":
    default:
      return "User";
  }
}

function effectiveMcpServerView(server) {
  var source = configScopeFromSource(server.source || "user_config");
  return {
    value: {
      id: server.id,
      name: server.name,
      transport: server.transport,
      enabled: server.enabled,
      runtime_status: server.runtime_status,
      trusted: server.trusted,
      tool_count: server.tool_count,
      last_error: server.last_error,
      writable: server.writable,
      config_path: server.config_path,
      description: server.description,
      source: server.source || "user_config",
      verified: server.verified ?? true
    },
    source: source,
    overrides: null,
    enabled: server.enabled,
    disabledBy: null,
    writable: server.writable,
    deletable: server.writable
  };
}

function effectiveSkillView(skill) {
  var source = configScopeFromSource(skill.scope);
  return {
    value: clone(skill),
    source: source,
    overrides: null,
    enabled: skill.enabled,
    disabledBy: null,
    writable: skill.editable,
    deletable: skill.deletable
  };
}

function getProjection(sessionId) {
  if (!state.projections.has(sessionId)) {
    state.projections.set(sessionId, {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
  }
  return state.projections.get(sessionId);
}

function getTrace(sessionId) {
  if (!state.traces.has(sessionId)) {
    state.traces.set(sessionId, []);
  }
  return state.traces.get(sessionId);
}

function makeEvent(sessionId, payload) {
  return {
    schema_version: 1,
    workspace_id: state.workspace ? state.workspace.workspace_id : "wrk_mock",
    session_id: sessionId,
    timestamp: new Date().toISOString(),
    source_agent_id: "agent_system",
    privacy: "full_trace",
    event_type: payload.type,
    payload: payload
  };
}

/**
 * Emit a Tauri-style event to all registered listeners.
 * In our mock, the handler was registered via transformCallback and stored
 * in our callbacks map. We invoke it directly.
 */
function emitEvent(eventName, payload) {
  var listeners = state.eventListeners.get(eventName);
  if (listeners) {
    listeners.forEach(function (handler, eventId) {
      setTimeout(function () {
        try {
          // Tauri v2 event handlers receive { event, id, payload }
          handler({ event: eventName, id: eventId, payload: payload });
        } catch (e) {
          console.error("[tauri-mock] Error in event listener for " + eventName + ":", e);
        }
      }, 10);
    });
  }
}

/* ---- transformCallback / unregisterCallback (Tauri v2 core) ---- */

function transformCallback(callback, once) {
  if (!callback) return 0;
  var id = state.nextCallbackId++;
  state.callbacks.set(id, { callback: callback, once: !!once });
  return id;
}

function unregisterCallback(id) {
  state.callbacks.delete(id);
}

function invokeCallback(id, args) {
  var entry = state.callbacks.get(id);
  if (entry) {
    if (entry.once) {
      state.callbacks.delete(id);
    }
    try {
      entry.callback.apply(null, args || []);
    } catch (e) {
      console.error("[tauri-mock] Error in callback " + id + ":", e);
    }
  }
}

/* ---- __TAURI_EVENT_PLUGIN_INTERNALS__ ---- */

var tauriEventPluginInternals = {
  unregisterListener: function (event, eventId) {
    var listeners = state.eventListeners.get(event);
    if (listeners) {
      listeners.delete(eventId);
    }
  }
};
