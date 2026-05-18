/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- session commands ---- */

registerCommandHandlers({
  start_session: function (args) {
    var profile = args.profile || "fast";
    var permissionMode = args.permissionMode || "suggest";
    var sid = nextId("ses");
    var session = makeSessionInfo(
      sid,
      "Session using " + profile,
      profile,
      null,
      null,
      null,
      "visible",
      permissionMode
    );
    state.currentPermissionMode = permissionMode;
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
  },
  send_message: function (args) {
    var sessionId = state.currentSessionId;
    if (!sessionId) return Promise.reject(new Error("No active session"));
    var content = args.content;
    state.sentMessages.push({
      sessionId: sessionId,
      content: content,
      attachments: args.attachments || []
    });
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
          total_tokens: 50000,
          budget_tokens: 100000,
          context_window: 128000,
          output_reservation: 28000,
          by_source: [
            ["system", 25000],
            ["history", 25000]
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
  },
  switch_session: function (args) {
    var sessionId = args.sessionId || args.session_id;
    if (!sessionId) return Promise.reject(new Error("sessionId is required"));
    state.currentSessionId = sessionId;
    var session = getSession(sessionId);
    if (session) {
      state.currentProfile = session.profile;
      if (session.permission_mode) state.currentPermissionMode = session.permission_mode;
    }
    return Promise.resolve(getProjection(sessionId));
  },
  get_trace: function (args) {
    var sessionId = args.sessionId || args.session_id;
    if (!sessionId) return Promise.reject(new Error("sessionId is required"));
    return Promise.resolve(
      getTrace(sessionId).map(function (e) {
        return JSON.stringify(e);
      })
    );
  },
  list_sessions: function (args) {
    return Promise.resolve(
      state.sessions.filter(function (session) {
        return !session.project_id && session.visibility === "visible";
      })
    );
  },
  rename_session: function (args) {
    var sessionId = args.sessionId || args.session_id;
    var title = args.title;
    var session = getSession(sessionId);
    if (session) session.title = title;
    return Promise.resolve(undefined);
  },
  delete_session: function (args) {
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
  },
  cancel_session: function (args) {
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
  },
  compact_session: function (args) {
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
  },
  switch_model: function (args) {
    var alias = args && (args.profileAlias || args.profile_alias);
    var reasoningEffort = args && (args.reasoningEffort || args.reasoning_effort || null);
    var switchSid = (args && (args.sessionId || args.session_id)) || state.currentSessionId;
    if (!alias) {
      return Promise.reject(new Error("profileAlias required"));
    }
    if (!switchSid) {
      return Promise.reject(new Error("No active session"));
    }
    var fromProfile = state.currentProfile;
    if (fromProfile === alias) {
      if (!reasoningEffort || reasoningEffort === state.currentReasoningEffort) {
        return Promise.resolve(null); // same-profile: silent no-op (mirrors runtime)
      }
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
    state.currentReasoningEffort = reasoningEffort;
    var switchedEvent = makeEvent(switchSid, {
      type: "ModelProfileSwitched",
      from_profile: fromProfile,
      to_profile: alias,
      reasoning_effort: reasoningEffort,
      effective_at: new Date().toISOString(),
      context_window: newWindow,
      output_limit: newOutput,
      limit_source: "builtin_registry"
    });
    getTrace(switchSid).push(switchedEvent);
    emitEvent("session-event", switchedEvent);
    return Promise.resolve(null);
  },
  get_task_graph: function (args) {
    var sessionId = args.sessionId || args.session_id;
    if (!sessionId) return Promise.reject(new Error("sessionId is required"));
    return Promise.resolve(getProjection(sessionId).task_graph.tasks);
  },
  get_permission_mode: function (args) {
    return Promise.resolve(state.currentPermissionMode);
  },
  set_permission_mode: function (args) {
    var mode = args.mode;
    if (!mode) return Promise.reject(new Error("mode is required"));
    state.currentPermissionMode = mode;
    var session = getSession(state.currentSessionId);
    if (session) session.permission_mode = mode;
    return Promise.resolve(mode);
  },
  resolve_permission: function (args) {
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
  },
  retry_task: function (args) {
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
  },
  cancel_task: function (args) {
    var taskId = args.taskId || args.task_id;
    var sessionId = state.currentSessionId;
    if (!sessionId) return Promise.reject(new Error("No active session"));
    var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
      return t.id === taskId;
    });
    if (task) {
      task.state = "Cancelled";
    }
    var event = makeEvent(sessionId, {
      type: "TaskCancelled",
      task_id: taskId
    });
    getTrace(sessionId).push(event);
    emitEvent("session-event", event);
    return Promise.resolve(undefined);
  }
});
