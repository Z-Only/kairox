/**
 * Tauri IPC mock runtime for Playwright E2E tests.
 *
 * The full mock is assembled by e2e/helpers/tauriMock.ts from the pure-JS
 * fragments in fixtures/tauri-mock plus this installer file. Do not import
 * from here directly in specs; call installTauriMock(page) instead.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- Install mock into window ---- */

function installMock() {
  // __TAURI_INTERNALS__ — the core IPC bridge used by @tauri-apps/api
  window.__TAURI_INTERNALS__ = {
    invoke: function (cmd, args, _options) {
      return invoke(cmd, args);
    },
    transformCallback: function (callback, once) {
      return transformCallback(callback, once);
    },
    unregisterCallback: function (id) {
      unregisterCallback(id);
    },
    convertFileSrc: function (filePath, _protocol) {
      return "http://localhost/asset/" + filePath;
    }
  };

  // __TAURI_EVENT_PLUGIN_INTERNALS__ — used by @tauri-apps/api/event.js
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = tauriEventPluginInternals;

  // Expose for test hooks
  window.__KAIROX_MOCK__ = {
    state: state,
    simulatePermissionRequest: function (toolId, preview) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var requestId = nextId("prm");
      state.permissionRequests.set(requestId, {
        tool_id: toolId,
        preview: preview
      });
      var event = makeEvent(sessionId, {
        type: "PermissionRequested",
        request_id: requestId,
        tool_id: toolId,
        preview: preview
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateMemoryProposal: function (scope, key, content) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var memoryId = nextId("mem");
      var event = makeEvent(sessionId, {
        type: "MemoryProposed",
        memory_id: memoryId,
        scope: scope,
        key: key,
        content: content
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return memoryId;
    },
    simulateTaskCreated: function (title, role) {
      role = role || "Worker";
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var taskId = nextId("tsk");
      var event = makeEvent(sessionId, {
        type: "AgentTaskCreated",
        task_id: taskId,
        title: title,
        role: role,
        dependencies: []
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return taskId;
    },
    simulateAgentSpawned: function (agentId, role, taskId) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      state.agents.set(agentId, {
        id: agentId,
        role: role,
        taskId: taskId || null,
        status: "running",
        startedAt: Date.now(),
        completedAt: null
      });
      var event = makeEvent(sessionId, {
        type: "AgentSpawned",
        agent_id: agentId,
        role: role,
        task_id: taskId || ""
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateAgentIdle: function (agentId) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var agent = state.agents.get(agentId);
      if (agent) {
        agent.status = "idle";
        agent.completedAt = Date.now();
      }
      var event = makeEvent(sessionId, {
        type: "AgentIdle",
        agent_id: agentId
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateTaskDecomposed: function (parentId, subTaskIds) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var event = makeEvent(sessionId, {
        type: "TaskDecomposed",
        parent_task_id: parentId,
        sub_task_ids: subTaskIds
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateTaskBlocked: function (taskId, blockingTaskId, reason) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Blocked";
        task.error = reason || "Dependency failed";
      }
      var event = makeEvent(sessionId, {
        type: "TaskBlocked",
        task_id: taskId,
        blocking_task_id: blockingTaskId,
        reason: reason || "Dependency failed"
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },

    simulateTaskTransition: function (taskId, eventType, error) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var payload = { type: eventType, task_id: taskId };
      if (eventType === "AgentTaskFailed" && error) payload.error = error;
      var event = makeEvent(sessionId, payload);
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    setNextOpenDialogResult: function (selected) {
      state.nextOpenDialogResult = selected;
    },
    setResponseDelayScale: function (scale) {
      state.responseDelayScale = scale || 1;
    },
    persistForReload: persistMockState,
    reset: function () {
      state.initialized = false;
      state.workspace = null;
      state.sessions = [];
      state.projects = [
        {
          project_id: "prj_mock",
          display_name: "Mock Project",
          root_path: "/mock/workspace",
          removed_at: null,
          sort_order: 0,
          expanded: true
        }
      ];
      state.projectSessions.clear();
      state.archivedSessions = [];
      state.gitStatuses.clear();
      state.currentSessionId = null;
      state.currentProfile = "fast";
      state.currentPermissionMode = "suggest";
      state.projections.clear();
      state.traces.clear();
      state.sentMessages = [];
      state.responseDelayScale = 1;
      state.memories = [];
      state.permissionRequests.clear();
      state.agents.clear();
      state.nextOpenDialogResult = null;
      state.drafts.clear();
      state.callbacks.clear();
      state.eventListeners.clear();
      _savedInstructions = { user: null, project: null };
      try {
        localStorage.removeItem(persistedStateKey);
      } catch {}
      idCounter = 0;
    }
  };
}

restorePersistedMockState();
installMock();
