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

function makeSessionInfo(
  id,
  title,
  profile,
  projectId,
  worktreePath,
  branch,
  visibility,
  permissionMode
) {
  return {
    id: id,
    title: title,
    profile: profile,
    permission_mode: permissionMode || "suggest",
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
        projectBranches: snapshotMap(state.projectBranches),
        currentSessionId: state.currentSessionId,
        currentProfile: state.currentProfile,
        currentPermissionMode: state.currentPermissionMode,
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
    state.projectBranches = new Map(snapshot.projectBranches || []);
    state.currentSessionId = snapshot.currentSessionId || null;
    state.currentProfile = snapshot.currentProfile || "fast";
    state.currentPermissionMode = snapshot.currentPermissionMode || "suggest";
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

function findAgentSetting(agentId) {
  var settingsIdMatches = state.agentSettings.filter(function (agent) {
    return agent.settingsId === agentId;
  });
  if (settingsIdMatches.length === 1) {
    return settingsIdMatches[0];
  }
  if (settingsIdMatches.length > 1) {
    throw new Error("ambiguous agent settings id: " + agentId);
  }

  var nameMatches = state.agentSettings.filter(function (agent) {
    return agent.name === agentId;
  });
  if (nameMatches.length === 1) {
    return nameMatches[0];
  }
  if (nameMatches.length > 1) {
    throw new Error("ambiguous agent name: " + agentId);
  }

  return null;
}

function agentSettingsPath(scope, name) {
  if (scope === "Builtin") return "builtin://" + name;
  if (scope === "Project") return "/mock/workspace/.kairox/agents/" + name + ".md";
  return "/Users/mock/.config/kairox/agents/" + name + ".md";
}

function refreshAgentSettingsEffectiveness() {
  var rank = { Builtin: 0, User: 1, Project: 2 };
  var grouped = new Map();
  state.agentSettings.forEach(function (agent) {
    if (!grouped.has(agent.name)) grouped.set(agent.name, []);
    grouped.get(agent.name).push(agent);
  });

  grouped.forEach(function (agents) {
    var winner = agents
      .filter(function (agent) {
        return agent.valid;
      })
      .sort(function (a, b) {
        return rank[b.scope] - rank[a.scope];
      })[0];
    agents.forEach(function (agent) {
      agent.effective = winner ? agent.settingsId === winner.settingsId : false;
      agent.shadowedBy = winner && !agent.effective ? winner.settingsId : null;
    });
  });
}

function createAgentSetting(input) {
  var name = slugify(input.name || "agent");
  var scope = input.scope || "User";
  return {
    settingsId: scope + ":" + name,
    name: name,
    description: input.description || "",
    scope: scope,
    path: agentSettingsPath(scope, name),
    tools: input.tools || [],
    modelProfile: input.modelProfile || null,
    permissionMode: input.permissionMode || null,
    skills: input.skills || [],
    nicknameCandidates: input.nicknameCandidates || [],
    enabled: input.enabled !== false,
    instructions: input.instructions || "",
    effective: true,
    shadowedBy: null,
    valid: true,
    validationError: null,
    editable: scope !== "Builtin",
    deletable: scope !== "Builtin"
  };
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
  var disabledByProject = state.disabledMcpServers.indexOf(server.id) >= 0;
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
    enabled: server.enabled && !disabledByProject,
    disabledBy: disabledByProject ? "Project" : null,
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

function findProjectionTask(sessionId, taskId) {
  return getProjection(sessionId).task_graph.tasks.find(function (task) {
    return task.id === taskId;
  });
}

function ensureProjectionTask(sessionId, input) {
  var task = findProjectionTask(sessionId, input.task_id);
  if (task) return task;

  task = {
    id: input.task_id,
    title: input.title,
    role: input.role || "Worker",
    state: "Pending",
    dependencies: input.dependencies || [],
    error: null,
    retry_count: 0,
    max_retries: 3,
    assigned_agent_id: null,
    failure_reason: null
  };
  getProjection(sessionId).task_graph.tasks.push(task);
  return task;
}

function applyTaskPayloadToProjection(sessionId, payload) {
  var task;
  switch (payload.type) {
    case "AgentTaskCreated":
      ensureProjectionTask(sessionId, payload);
      break;
    case "AgentTaskStarted":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) task.state = "Running";
      break;
    case "AgentTaskCompleted":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) task.state = "Completed";
      break;
    case "AgentTaskFailed":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) {
        task.state = "Failed";
        task.error = payload.error || null;
      }
      break;
    case "TaskBlocked":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) {
        task.state = "Blocked";
        task.error = payload.reason || "Dependency failed";
      }
      break;
    case "TaskRetried":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) {
        task.state = "Running";
        task.retry_count = payload.attempt;
        task.error = null;
      }
      break;
    case "TaskCancelled":
      task = findProjectionTask(sessionId, payload.task_id);
      if (task) {
        task.state = "Cancelled";
        task.error = null;
      }
      break;
  }
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
