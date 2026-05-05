/**
 * Tauri IPC mock for Playwright E2E tests (PURE JAVASCRIPT).
 *
 * This file is injected via Playwright's addInitScript BEFORE any page JS runs.
 * It MUST be valid JavaScript (no TypeScript) because the browser evaluates it directly.
 *
 * It replaces @tauri-apps/api by providing a full __TAURI_INTERNALS__ shim
 * that the Tauri v2 API library uses internally, plus __TAURI_EVENT_PLUGIN_INTERNALS__.
 */

// @ts-nocheck

/* ---- State ---- */

let idCounter = 0;
function nextId(prefix) {
  return prefix + "_" + ++idCounter;
}

const state = {
  initialized: false,
  workspace: null,
  sessions: [],
  currentSessionId: null,
  currentProfile: "fast",
  projections: new Map(),
  traces: new Map(),
  memories: [],
  permissionRequests: new Map(),
  /** Tauri v2 event system: eventName → Map<eventId, handler> */
  eventListeners: new Map(),
  profiles: [
    {
      alias: "fast",
      provider: "openai",
      model_id: "gpt-4o-mini",
      local: false,
      has_api_key: true
    },
    {
      alias: "smart",
      provider: "openai",
      model_id: "gpt-4o",
      local: false,
      has_api_key: true
    },
    {
      alias: "fake",
      provider: "fake",
      model_id: "fake-model",
      local: true,
      has_api_key: false
    }
  ],
  /** Callback registry for transformCallback */
  callbacks: new Map(),
  nextCallbackId: 1
};

/* ---- Helpers ---- */

function getSession(sessionId) {
  const id = sessionId || state.currentSessionId;
  return state.sessions.find(function (s) {
    return s.id === id;
  });
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
          console.error(
            "[tauri-mock] Error in event listener for " + eventName + ":",
            e
          );
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

/* ---- invoke handler ---- */

function invoke(cmd, args) {
  args = args || {};

  switch (cmd) {
    /* ─── Tauri v2 Event Plugin ──────────────────────────────── */
    case "plugin:event|listen": {
      var eventName = args.event;
      var handlerId = args.handler;
      if (!state.eventListeners.has(eventName)) {
        state.eventListeners.set(eventName, new Map());
      }
      // Store the wrapped handler that will be called when we emitEvent
      state.eventListeners.get(eventName).set(handlerId, function (evt) {
        // Invoke the callback that was registered via transformCallback
        invokeCallback(handlerId, [evt]);
      });
      // Return the handlerId as the eventId (this is what Tauri v2 returns)
      return Promise.resolve(handlerId);
    }

    case "plugin:event|unlisten": {
      var eventName = args.event;
      var eventId = args.eventId;
      var listeners = state.eventListeners.get(eventName);
      if (listeners) {
        listeners.delete(eventId);
      }
      return Promise.resolve(undefined);
    }

    /* ─── App commands ───────────────────────────────────────── */

    case "initialize_workspace": {
      if (state.initialized)
        return Promise.reject(new Error("Workspace already initialized"));
      var ws = { workspace_id: "wrk_mock", path: "/mock/workspace" };
      state.workspace = ws;
      state.initialized = true;
      // Auto-create a first session
      var sid = nextId("ses");
      var session = { id: sid, title: "Session using fast", profile: "fast" };
      state.sessions.push(session);
      state.currentSessionId = sid;
      state.projections.set(sid, {
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false
      });
      state.traces.set(sid, []);
      return Promise.resolve(ws);
    }

    case "list_profiles":
      return Promise.resolve(
        state.profiles.map(function (p) {
          return p.alias;
        })
      );

    case "get_profile_info":
      return Promise.resolve(state.profiles);

    case "get_profile_detail": {
      var profile = args.profile || "fast";
      var found = state.profiles.find(function (p) {
        return p.alias === profile;
      });
      if (!found)
        return Promise.reject(new Error("Profile '" + profile + "' not found"));
      return Promise.resolve(found);
    }

    case "start_session": {
      var profile = args.profile || "fast";
      var sid = nextId("ses");
      var session = {
        id: sid,
        title: "Session using " + profile,
        profile: profile
      };
      state.sessions.push(session);
      state.currentSessionId = sid;
      state.projections.set(sid, {
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false
      });
      state.traces.set(sid, []);
      // Emit SessionInitialized event
      var event = makeEvent(sid, {
        type: "SessionInitialized",
        model_profile: profile
      });
      getTrace(sid).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(session);
    }

    case "send_message": {
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var content = args.content;
      var projection = getProjection(sessionId);
      var trace = getTrace(sessionId);
      // UserMessageAdded
      var userMsgId = nextId("msg");
      var userEvent = makeEvent(sessionId, {
        type: "UserMessageAdded",
        message_id: userMsgId,
        content: content
      });
      trace.push(userEvent);
      emitEvent("session-event", userEvent);
      // Simulate agent response asynchronously
      setTimeout(function () {
        var ctxEvent = makeEvent(sessionId, {
          type: "ContextAssembled",
          token_estimate: 256,
          sources: ["system_prompt", "conversation_history"]
        });
        trace.push(ctxEvent);
        emitEvent("session-event", ctxEvent);
        var modelEvent = makeEvent(sessionId, {
          type: "ModelRequestStarted",
          model_profile: state.currentProfile,
          model_id: "gpt-4o-mini"
        });
        trace.push(modelEvent);
        emitEvent("session-event", modelEvent);
        var tokens = ["Hello! ", "I'm a mock ", "assistant."];
        var delay = 50;
        for (var i = 0; i < tokens.length; i++) {
          (function (token, d) {
            setTimeout(function () {
              var deltaEvent = makeEvent(sessionId, {
                type: "ModelTokenDelta",
                delta: token
              });
              trace.push(deltaEvent);
              emitEvent("session-event", deltaEvent);
            }, d);
          })(tokens[i], delay);
          delay += 100;
        }
        setTimeout(function () {
          var assistantMsgId = nextId("msg");
          var fullContent = "Hello! I'm a mock assistant.";
          var completedEvent = makeEvent(sessionId, {
            type: "AssistantMessageCompleted",
            message_id: assistantMsgId,
            content: fullContent
          });
          trace.push(completedEvent);
          emitEvent("session-event", completedEvent);
        }, delay + 50);
      }, 30);
      return Promise.resolve(undefined);
    }

    case "switch_session": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      state.currentSessionId = sessionId;
      var session = getSession(sessionId);
      if (session) state.currentProfile = session.profile;
      return Promise.resolve(getProjection(sessionId));
    }

    case "get_trace": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      return Promise.resolve(
        getTrace(sessionId).map(function (e) {
          return JSON.stringify(e);
        })
      );
    }

    case "list_sessions":
      return Promise.resolve(state.sessions);

    case "list_workspaces":
      return Promise.resolve(state.workspace ? [state.workspace] : []);

    case "restore_workspace": {
      var workspaceId = args.workspaceId || args.workspace_id;
      if (state.sessions.length > 0) {
        state.currentSessionId = state.sessions[0].id;
      }
      return Promise.resolve(undefined);
    }

    case "resolve_permission": {
      var requestId = args.requestId || args.request_id;
      var decision = args.decision;
      var request = state.permissionRequests.get(requestId);
      if (!request)
        return Promise.reject(
          new Error("Permission request " + requestId + " not found")
        );
      var sessionId = state.currentSessionId;
      if (sessionId) {
        if (decision === "grant") {
          var event = makeEvent(sessionId, {
            type: "PermissionGranted",
            request_id: requestId
          });
          getTrace(sessionId).push(event);
          emitEvent("session-event", event);
          setTimeout(function () {
            var invId = nextId("inv");
            var startEvent = makeEvent(sessionId, {
              type: "ToolInvocationStarted",
              invocation_id: invId,
              tool_id: request.tool_id
            });
            getTrace(sessionId).push(startEvent);
            emitEvent("session-event", startEvent);
            setTimeout(function () {
              var compEvent = makeEvent(sessionId, {
                type: "ToolInvocationCompleted",
                invocation_id: invId,
                tool_id: request.tool_id,
                output_preview: "Output of " + request.tool_id,
                exit_code: 0,
                duration_ms: 150,
                truncated: false
              });
              getTrace(sessionId).push(compEvent);
              emitEvent("session-event", compEvent);
            }, 100);
          }, 50);
        } else {
          var event = makeEvent(sessionId, {
            type: "PermissionDenied",
            request_id: requestId,
            reason: args.reason || "User denied"
          });
          getTrace(sessionId).push(event);
          emitEvent("session-event", event);
        }
      }
      state.permissionRequests.delete(requestId);
      return Promise.resolve(undefined);
    }

    case "query_memories": {
      var results = state.memories.slice();
      var scope = args.scope || null;
      if (scope)
        results = results.filter(function (m) {
          return m.scope === scope;
        });
      var keywords = args.keywords || null;
      if (keywords && keywords.length > 0) {
        results = results.filter(function (m) {
          return keywords.some(function (k) {
            return m.content.toLowerCase().indexOf(k.toLowerCase()) !== -1;
          });
        });
      }
      var limit = args.limit || 50;
      return Promise.resolve(results.slice(0, limit));
    }

    case "delete_memory": {
      var id = args.id;
      state.memories = state.memories.filter(function (m) {
        return m.id !== id;
      });
      return Promise.resolve(undefined);
    }

    case "rename_session": {
      var sessionId = args.sessionId || args.session_id;
      var title = args.title;
      var session = getSession(sessionId);
      if (session) session.title = title;
      return Promise.resolve(undefined);
    }

    case "delete_session": {
      var sessionId = args.sessionId || args.session_id;
      state.sessions = state.sessions.filter(function (s) {
        return s.id !== sessionId;
      });
      state.projections.delete(sessionId);
      state.traces.delete(sessionId);
      if (state.currentSessionId === sessionId) {
        state.currentSessionId =
          state.sessions.length > 0 ? state.sessions[0].id : null;
      }
      return Promise.resolve(undefined);
    }

    case "cancel_session": {
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var projection = getProjection(sessionId);
      projection.cancelled = true;
      var event = makeEvent(sessionId, {
        type: "SessionCancelled",
        reason: "User cancelled"
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(undefined);
    }

    case "get_task_graph": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      return Promise.resolve(getProjection(sessionId).task_graph.tasks);
    }

    case "get_permission_mode":
      return Promise.resolve("Interactive");

    case "get_build_info":
      return Promise.resolve({
        version: "0.12.0-e2e",
        git_hash: "mock",
        build_time: "2026-05-05"
      });

    default:
      console.warn("[tauri-mock] Unknown invoke: " + cmd, args);
      return Promise.resolve(undefined);
  }
}

/* ---- Install mock into window ---- */

function installMock() {
  // __TAURI_INTERNALS__ — the core IPC bridge used by @tauri-apps/api
  window.__TAURI_INTERNALS__ = {
    invoke: function (cmd, args, options) {
      return invoke(cmd, args);
    },
    transformCallback: function (callback, once) {
      return transformCallback(callback, once);
    },
    unregisterCallback: function (id) {
      unregisterCallback(id);
    },
    convertFileSrc: function (filePath, protocol) {
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
    simulateTaskTransition: function (taskId, eventType, error) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var payload = { type: eventType, task_id: taskId };
      if (eventType === "AgentTaskFailed" && error) payload.error = error;
      var event = makeEvent(sessionId, payload);
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    reset: function () {
      state.initialized = false;
      state.workspace = null;
      state.sessions = [];
      state.currentSessionId = null;
      state.currentProfile = "fast";
      state.projections.clear();
      state.traces.clear();
      state.memories = [];
      state.permissionRequests.clear();
      state.callbacks.clear();
      state.eventListeners.clear();
      idCounter = 0;
    }
  };
}

installMock();
