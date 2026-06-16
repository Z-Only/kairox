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
    commandCalls: function (command) {
      return state.commandInvocations
        .filter(function (call) {
          return call.command === command;
        })
        .map(function (call) {
          return clone(call);
        });
    },
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
    simulateTaskConfirmationRequest: function (prompt, options, allowMultiple, allowCustom) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var requestId = nextId("tcr");
      var normalizedOptions = options || [
        {
          id: "default",
          label: "Default option",
          description: null
        }
      ];
      state.taskConfirmationRequests.set(requestId, {
        prompt: prompt,
        options: normalizedOptions,
        allow_multiple: Boolean(allowMultiple),
        allow_custom: Boolean(allowCustom)
      });
      var event = makeEvent(sessionId, {
        type: "TaskConfirmationRequested",
        request_id: requestId,
        prompt: prompt,
        options: normalizedOptions,
        allow_multiple: Boolean(allowMultiple),
        allow_custom: Boolean(allowCustom)
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return requestId;
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
    simulateTaskCreated: function (title, role, dependencies) {
      role = role || "Worker";
      dependencies = dependencies || [];
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var taskId = nextId("tsk");
      var payload = {
        type: "AgentTaskCreated",
        task_id: taskId,
        title: title,
        role: role,
        dependencies: dependencies
      };
      applyTaskPayloadToProjection(sessionId, payload);
      var event = makeEvent(sessionId, payload);
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
      var payload = {
        type: "TaskBlocked",
        task_id: taskId,
        blocking_task_id: blockingTaskId,
        reason: reason || "Dependency failed"
      };
      applyTaskPayloadToProjection(sessionId, payload);
      var event = makeEvent(sessionId, payload);
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },

    simulateTaskTransition: function (taskId, eventType, error) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var payload = { type: eventType, task_id: taskId };
      if (eventType === "AgentTaskFailed" && error) payload.error = error;
      applyTaskPayloadToProjection(sessionId, payload);
      var event = makeEvent(sessionId, payload);
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateUserMessage: function (content) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return null;
      var messageId = nextId("msg");
      var event = makeEvent(sessionId, {
        type: "UserMessageAdded",
        message_id: messageId,
        content: content
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return messageId;
    },
    simulateAssistantMessage: function (content) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return null;
      var messageId = nextId("msg");
      var event = makeEvent(sessionId, {
        type: "AssistantMessageCompleted",
        message_id: messageId,
        content: content
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return messageId;
    },
    simulateModelToolCallRequested: function (toolId, toolCallId) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return null;
      var id = toolCallId || nextId("call");
      var event = makeEvent(sessionId, {
        type: "ModelToolCallRequested",
        tool_call_id: id,
        tool_id: toolId
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return id;
    },
    /**
     * Emit ToolInvocationStarted + ToolInvocationCompleted for a single
     * invocation. The trace reducer keys these by `invocation_id`, so
     * Completed updates the same entry with `output_preview` and
     * `duration_ms` — the fields the chat-stream's tool-call row reveals
     * after expand. Returns the invocation id used.
     */
    simulateToolInvocation: function (toolId, outputPreview, durationMs) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return null;
      var invId = nextId("inv");
      var startEvent = makeEvent(sessionId, {
        type: "ToolInvocationStarted",
        invocation_id: invId,
        tool_id: toolId
      });
      getTrace(sessionId).push(startEvent);
      emitEvent("session-event", startEvent);
      var compEvent = makeEvent(sessionId, {
        type: "ToolInvocationCompleted",
        invocation_id: invId,
        tool_id: toolId,
        output_preview: outputPreview || "",
        exit_code: 0,
        duration_ms: durationMs == null ? 150 : durationMs,
        truncated: false
      });
      getTrace(sessionId).push(compEvent);
      emitEvent("session-event", compEvent);
      return invId;
    },
    /**
     * Drive `session.projection.compaction` directly so the chat-stream's
     * ChatCompactionItem renders. The live session-event reducer only
     * toggles a `compacting` boolean for ContextCompactionStarted/Completed/
     * Failed — it never writes `projection.compaction`, which is only
     * updated through snapshot loads (`setProjection`). The Rust enum has
     * no `Completed` variant, but ChatCompactionItem accepts a local
     * extended `{ type: "Completed" }` for the completed visual state.
     */
    simulateCompactionStatus: function (statusPatch) {
      var mountedAppElement = document.querySelector("#app");
      if (!mountedAppElement || !mountedAppElement.__vue_app__) return;
      var pinia = mountedAppElement.__vue_app__.config.globalProperties.$pinia;
      if (!pinia || !pinia._s) return;
      var sessionStore = pinia._s.get("session");
      if (!sessionStore) return;
      var nextProjection = Object.assign({}, sessionStore.projection, {
        compaction: statusPatch
      });
      sessionStore.setProjection(nextProjection);
      var sessionId = state.currentSessionId;
      if (sessionId) {
        var eventType =
          statusPatch && statusPatch.type === "Running"
            ? "ContextCompactionStarted"
            : statusPatch && statusPatch.type === "Failed"
              ? "ContextCompactionFailed"
              : "ContextCompactionCompleted";
        var payload = { type: eventType };
        if (eventType === "ContextCompactionStarted") {
          payload.reason = { type: "UserRequested" };
          payload.before_tokens = 12000;
          payload.candidate_event_count = 4;
        } else if (eventType === "ContextCompactionCompleted") {
          payload.summary_id = "sum_mock_e2e";
          payload.after_tokens = 3000;
          payload.fallback_used = false;
        } else {
          payload.error = (statusPatch && statusPatch.error) || "Compaction failed";
        }
        var event = makeEvent(sessionId, payload);
        getTrace(sessionId).push(event);
        emitEvent("session-event", event);
      }
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
      state.projectBranches = new Map([["prj_mock", ["main", "develop"]]]);
      state.currentSessionId = null;
      state.currentProfile = "fast";
      state.currentApprovalPolicy = "on_request";
      state.currentSandboxPolicy = '{"kind":"workspace_write"}';
      state.projections.clear();
      state.traces.clear();
      state.sentMessages = [];
      state.responseDelayScale = 1;
      state.memories = [];
      state.permissionRequests.clear();
      state.taskConfirmationRequests.clear();
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
