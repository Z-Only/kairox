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
  agents: new Map(),
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
  nextCallbackId: 1,
  /** Marketplace fixtures (Phase 1 builtin catalog) */
  catalog: [
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
  ],
  installedCatalog: [],
  catalogRuntimePresent: { node: true, python: true, uvx: true, docker: true },
  // Phase 2: catalog sources — only user-configured remote sources are listed here.
  // The builtin source is implicit (the GUI's source chip bar always renders a
  // "Built-in" chip in addition to whatever list_catalog_sources returns).
  catalogSources: []
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
      if (state.initialized) return Promise.reject(new Error("Workspace already initialized"));
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

    case "list_profiles_with_limits":
      return Promise.resolve(
        state.profiles.map(function (p) {
          var window;
          var output;
          if (p.alias === "fast") {
            window = 128000;
            output = 16384;
          } else if (p.alias === "smart") {
            window = 200000;
            output = 16384;
          } else {
            window = 4096;
            output = 2048;
          }
          return {
            alias: p.alias,
            provider: p.provider,
            model_id: p.model_id,
            context_window: window,
            output_limit: output,
            limit_source: "builtin_registry",
            has_api_key: p.has_api_key
          };
        })
      );

    case "get_profile_info":
      return Promise.resolve(state.profiles);

    case "get_profile_detail": {
      var profile = args.profile || "fast";
      var found = state.profiles.find(function (p) {
        return p.alias === profile;
      });
      if (!found) return Promise.reject(new Error("Profile '" + profile + "' not found"));
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
          usage: {
            total_tokens: 256,
            budget_tokens: 100000,
            context_window: 128000,
            output_reservation: 28000,
            by_source: [
              ["system", 128],
              ["history", 128]
            ],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
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
      var _workspaceId = args.workspaceId || args.workspace_id;
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
        return Promise.reject(new Error("Permission request " + requestId + " not found"));
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
        state.currentSessionId = state.sessions.length > 0 ? state.sessions[0].id : null;
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

    case "compact_session": {
      var sid = state.currentSessionId;
      if (!sid) return Promise.reject(new Error("No active session"));
      var startedEvent = makeEvent(sid, {
        type: "ContextCompactionStarted",
        reason: { type: "UserRequested" },
        before_tokens: 12000,
        candidate_event_count: 4
      });
      getTrace(sid).push(startedEvent);
      emitEvent("session-event", startedEvent);
      setTimeout(function () {
        var summaryEvent = makeEvent(sid, {
          type: "CompactionSummary",
          summary_id: "sum_mock_1",
          content: "## User goal\nMock summary content for E2E.",
          replaces_event_range: [new Date().toISOString(), new Date().toISOString()],
          reason: { type: "UserRequested" },
          before_tokens: 12000,
          after_tokens: 3000,
          summarised_by_profile: state.currentProfile
        });
        getTrace(sid).push(summaryEvent);
        emitEvent("session-event", summaryEvent);
        var completedEvent = makeEvent(sid, {
          type: "ContextCompactionCompleted",
          summary_id: "sum_mock_1",
          after_tokens: 3000,
          fallback_used: false
        });
        getTrace(sid).push(completedEvent);
        emitEvent("session-event", completedEvent);
      }, 100);
      return Promise.resolve(undefined);
    }

    case "switch_model": {
      var alias = args && (args.profileAlias || args.profile_alias);
      var switchSid = (args && (args.sessionId || args.session_id)) || state.currentSessionId;
      if (!alias) {
        return Promise.reject(new Error("profileAlias required"));
      }
      if (!switchSid) {
        return Promise.reject(new Error("No active session"));
      }
      var fromProfile = state.currentProfile;
      if (fromProfile === alias) {
        return Promise.resolve(null); // same-profile: silent no-op (mirrors runtime)
      }
      // Resolve new limits from the same table list_profiles_with_limits uses.
      var newWindow;
      var newOutput;
      if (alias === "fast") {
        newWindow = 128000;
        newOutput = 16384;
      } else if (alias === "smart") {
        newWindow = 200000;
        newOutput = 16384;
      } else {
        // Unknown alias → reject like the real runtime does
        // (agent-core::CoreError::InvalidState).
        return Promise.reject(new Error("Unknown model profile: " + alias));
      }
      state.currentProfile = alias;
      var switchedEvent = makeEvent(switchSid, {
        type: "ModelProfileSwitched",
        from_profile: fromProfile,
        to_profile: alias,
        effective_at: new Date().toISOString(),
        context_window: newWindow,
        output_limit: newOutput,
        limit_source: "builtin_registry"
      });
      getTrace(switchSid).push(switchedEvent);
      emitEvent("session-event", switchedEvent);
      return Promise.resolve(null);
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

    case "list_mcp_servers":
      return [
        { id: "test-server", status: "running", tool_count: 3 },
        { id: "stopped-server", status: "stopped", tool_count: 0 }
      ];
    case "start_mcp_server":
      return null;
    case "stop_mcp_server":
      return null;
    case "trust_mcp_server":
      return null;
    case "revoke_mcp_trust":
      return null;
    case "refresh_mcp_tools":
      return [{ name: "echo", description: "Echo tool", input_schema: null }];
    case "list_mcp_resources":
      return [];
    case "list_mcp_prompts":
      return [];
    case "read_mcp_resource":
      return [];

    /* ─── Marketplace commands ───────────────────────────────── */

    case "list_catalog": {
      return state.catalog;
    }

    case "get_catalog_entry": {
      var ce = state.catalog.find(function (e) {
        return e.id === args.id;
      });
      return ce || null;
    }

    case "refresh_catalog": {
      var refreshSource = args.source || "aggregate";
      var refreshSession = state.currentSessionId;
      if (refreshSession) {
        var refreshEvent = makeEvent(refreshSession, {
          type: "CatalogRefreshed",
          source: refreshSource,
          entry_count: state.catalog.length
        });
        getTrace(refreshSession).push(refreshEvent);
        emitEvent("session-event", refreshEvent);
      }
      return null;
    }

    case "install_catalog_entry": {
      var req = args.request;
      var entry = state.catalog.find(function (e) {
        return e.id === req.catalog_id;
      });
      if (!entry) {
        return Promise.reject(new Error("catalog entry not found: " + req.catalog_id));
      }
      var reqs = JSON.parse(entry.requirements_json);
      var baseMissing = reqs
        .filter(function (r) {
          return !state.catalogRuntimePresent[r.kind];
        })
        .map(function (r) {
          return r.kind;
        });
      // Test hook: e2e specs may set window.__MARKETPLACE_FORCE_MISSING__
      // to a string[] of runtime kinds to force a runtime_missing outcome.
      var forced = (typeof window !== "undefined" && window.__MARKETPLACE_FORCE_MISSING__) || null;
      var missing = forced && Array.isArray(forced) && forced.length > 0 ? forced : baseMissing;
      var sessionId = state.currentSessionId;
      if (missing.length > 0) {
        if (sessionId) {
          var missingEvent = makeEvent(sessionId, {
            type: "CatalogRuntimeMissing",
            catalog_id: req.catalog_id,
            missing: missing
          });
          getTrace(sessionId).push(missingEvent);
          emitEvent("session-event", missingEvent);
        }
        return {
          kind: "runtime_missing",
          server_id: null,
          started: null,
          missing_runtimes: missing,
          missing_env_keys: []
        };
      }
      var defaults = JSON.parse(entry.default_env_json);
      var missingEnv = defaults
        .filter(function (d) {
          return d.required && !req.env_overrides[d.key] && !d.default;
        })
        .map(function (d) {
          return d.key;
        });
      if (missingEnv.length > 0) {
        return {
          kind: "invalid_env",
          server_id: null,
          started: null,
          missing_runtimes: [],
          missing_env_keys: missingEnv
        };
      }
      if (
        state.installedCatalog.find(function (e) {
          return e.server_id === req.catalog_id;
        })
      ) {
        return {
          kind: "already_installed",
          server_id: req.catalog_id,
          started: null,
          missing_runtimes: [],
          missing_env_keys: []
        };
      }
      state.installedCatalog.push({
        server_id: req.catalog_id,
        catalog_id: req.catalog_id,
        source: req.source,
        display_name: entry.display_name,
        installed_at: new Date().toISOString(),
        running: !!req.auto_start
      });
      if (sessionId) {
        var installingEvent = makeEvent(sessionId, {
          type: "CatalogEntryInstalling",
          catalog_id: req.catalog_id,
          source: req.source
        });
        getTrace(sessionId).push(installingEvent);
        emitEvent("session-event", installingEvent);
        var installedEvent = makeEvent(sessionId, {
          type: "CatalogEntryInstalled",
          catalog_id: req.catalog_id,
          source: req.source,
          server_id: req.catalog_id
        });
        getTrace(sessionId).push(installedEvent);
        emitEvent("session-event", installedEvent);
      }
      return {
        kind: "installed",
        server_id: req.catalog_id,
        started: !!req.auto_start,
        missing_runtimes: [],
        missing_env_keys: []
      };
    }

    case "uninstall_catalog_entry": {
      var uninstSession = state.currentSessionId;
      state.installedCatalog = state.installedCatalog.filter(function (e) {
        return e.server_id !== args.serverId;
      });
      if (uninstSession) {
        var uninstEvent = makeEvent(uninstSession, {
          type: "CatalogEntryUninstalled",
          server_id: args.serverId
        });
        getTrace(uninstSession).push(uninstEvent);
        emitEvent("session-event", uninstEvent);
      }
      return null;
    }

    case "list_installed_entries": {
      return state.installedCatalog;
    }

    case "retry_task": {
      var taskId = args.taskId || args.task_id;
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Running";
        task.retry_count = (task.retry_count || 0) + 1;
      }
      var event = makeEvent(sessionId, {
        type: "TaskRetried",
        task_id: taskId,
        attempt: task ? task.retry_count : 1
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(undefined);
    }

    case "cancel_task": {
      var taskId = args.taskId || args.task_id;
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Cancelled";
      }
      return Promise.resolve(undefined);
    }

    /* ─── Phase 2: catalog source commands ───────────────────── */

    case "list_catalog_sources": {
      return state.catalogSources.slice();
    }

    case "add_catalog_source": {
      var addReq = args.request;
      if (
        state.catalogSources.find(function (s) {
          return s.id === addReq.id;
        })
      ) {
        return Promise.reject(new Error("source already exists: " + addReq.id));
      }
      state.catalogSources.push({
        id: addReq.id,
        display_name: addReq.display_name,
        kind: addReq.kind,
        url: addReq.url,
        api_key_env: addReq.api_key_env || null,
        priority: addReq.priority != null ? addReq.priority : 100,
        default_trust: addReq.default_trust || "community",
        enabled: addReq.enabled != null ? addReq.enabled : true,
        cache_ttl_seconds: addReq.cache_ttl_seconds || null,
        last_error: null
      });
      var addSession = state.currentSessionId;
      if (addSession) {
        var addEvent = makeEvent(addSession, {
          type: "CatalogSourceAdded",
          source: addReq.id,
          kind: addReq.kind
        });
        getTrace(addSession).push(addEvent);
        emitEvent("session-event", addEvent);
      }
      return null;
    }

    case "remove_catalog_source": {
      var removeId = args.id;
      if (removeId === "builtin") return null;
      state.catalogSources = state.catalogSources.filter(function (s) {
        return s.id !== removeId;
      });
      return null;
    }

    case "set_catalog_source_enabled": {
      var setId = args.id;
      var setEnabled = args.enabled;
      state.catalogSources = state.catalogSources.map(function (s) {
        return s.id === setId ? Object.assign({}, s, { enabled: setEnabled }) : s;
      });
      return null;
    }

    default:
      console.warn("[tauri-mock] Unknown invoke: " + cmd, args);
      return Promise.resolve(undefined);
  }
}

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
      state.agents.clear();
      state.callbacks.clear();
      state.eventListeners.clear();
      idCounter = 0;
    }
  };
}

installMock();
