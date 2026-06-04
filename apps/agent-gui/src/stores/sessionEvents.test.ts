import { describe, it, expect, beforeEach, vi } from "vitest";
import { ref, type Ref } from "vue";
import type {
  SessionProjection,
  DomainEvent,
  EventPayload,
  ContextUsage,
  ProjectedModelLimits,
  AgentRole
} from "@/types";
import {
  emptyProjection,
  appendAssistantErrorMessage,
  applySessionEvent,
  setProjectionFromSnapshot,
  resetProjectionState,
  type EventReducerContext
} from "@/stores/sessionEvents";

function makeCtx(overrides: Partial<EventReducerContext> = {}): EventReducerContext {
  return {
    projection: ref(emptyProjection()) as Ref<SessionProjection>,
    isStreaming: ref(false),
    lastSendError: ref(null) as Ref<string | null>,
    lastContextUsage: ref(null) as Ref<ContextUsage | null>,
    compacting: ref(false),
    lastCompactionError: ref(null) as Ref<string | null>,
    currentProfile: ref("fast"),
    currentReasoningEffort: ref(null) as Ref<string | null>,
    modelLimits: ref(null) as Ref<ProjectedModelLimits | null>,
    ...overrides
  };
}

function makeEvent(payload: EventPayload, sourceAgentId = "agent_system"): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_test",
    session_id: "ses_test",
    timestamp: "2026-05-06T00:00:00Z",
    source_agent_id: sourceAgentId,
    privacy: "full_trace",
    event_type: payload.type,
    payload
  } as DomainEvent;
}

function makeAgentsStore(agents: Map<string, { id: string; role: AgentRole }> = new Map()) {
  return {
    agents,
    clearAgents: vi.fn(),
    applyAgentEvent: vi.fn()
  } as unknown as ReturnType<typeof import("@/stores/agents").useAgentsStore>;
}

function makeTaskGraphStore() {
  return {
    tasks: [],
    currentSessionId: null,
    loading: false,
    setTaskGraph: vi.fn(),
    clearTaskGraph: vi.fn(),
    applyTaskEvent: vi.fn()
  } as unknown as ReturnType<typeof import("@/stores/taskGraph").useTaskGraphStore>;
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// emptyProjection
// ---------------------------------------------------------------------------
describe("emptyProjection", () => {
  it("returns a clean session projection", () => {
    const p = emptyProjection();
    expect(p.messages).toEqual([]);
    expect(p.task_titles).toEqual([]);
    expect(p.task_graph).toEqual({ tasks: [] });
    expect(p.token_stream).toBe("");
    expect(p.cancelled).toBe(false);
    expect(p.last_context_usage).toBeNull();
    expect(p.model_limits).toBeNull();
    expect(p.compaction).toEqual({ type: "Idle" });
  });

  it("returns a fresh object on each call (no shared references)", () => {
    const a = emptyProjection();
    const b = emptyProjection();
    expect(a).not.toBe(b);
    expect(a.messages).not.toBe(b.messages);
    a.messages.push({ role: "user", content: "test" });
    expect(b.messages).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — message events
// ---------------------------------------------------------------------------
describe("applySessionEvent — message events", () => {
  it("projects UserMessageAdded", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "hello" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages).toHaveLength(1);
    expect(ctx.projection.value.messages[0]).toEqual({ role: "user", content: "hello" });
    expect(ctx.isStreaming.value).toBe(true);
    expect(ctx.lastSendError.value).toBeNull();
  });

  it("uses display_content for attached UserMessageAdded events", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "UserMessageAdded",
        message_id: "m1",
        content: "```md\n// file: notes.md\nsecret\n```",
        display_content: "@notes.md summarize this"
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages).toEqual([
      { role: "user", content: "@notes.md summarize this" }
    ]);
  });

  it("clears lastSendError on UserMessageAdded", () => {
    const ctx = makeCtx({ lastSendError: ref("previous error") as Ref<string | null> });
    applySessionEvent(
      makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "retry" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.lastSendError.value).toBeNull();
  });

  it("clears the previous cancellation marker on UserMessageAdded", () => {
    const projection = emptyProjection();
    projection.cancelled = true;
    projection.token_stream = "stale cancelled tokens";
    const ctx = makeCtx({
      projection: ref(projection) as Ref<SessionProjection>,
      isStreaming: ref(false)
    });

    applySessionEvent(
      makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "continue" }),
      ctx,
      makeAgentsStore()
    );

    expect(ctx.projection.value.cancelled).toBe(false);
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.isStreaming.value).toBe(true);
  });

  it("accumulates ModelTokenDelta into token_stream", () => {
    const ctx = makeCtx();
    const agents = makeAgentsStore();
    applySessionEvent(makeEvent({ type: "ModelTokenDelta", delta: "hel" }), ctx, agents);
    applySessionEvent(makeEvent({ type: "ModelTokenDelta", delta: "lo" }), ctx, agents);
    expect(ctx.projection.value.token_stream).toBe("hello");
  });

  it("finalizes on AssistantMessageCompleted", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({ type: "AssistantMessageCompleted", message_id: "m2", content: "reply" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages).toHaveLength(1);
    expect(ctx.projection.value.messages[0]).toEqual({ role: "assistant", content: "reply" });
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.isStreaming.value).toBe(false);
  });

  it("attributes AssistantMessageCompleted to a known agent", () => {
    const ctx = makeCtx();
    const agents = makeAgentsStore(
      new Map([
        [
          "agent_w1",
          {
            id: "agent_w1",
            role: "Worker" as AgentRole,
            taskId: "t1",
            status: "running",
            startedAt: Date.now(),
            completedAt: null
          }
        ]
      ]) as never
    );

    applySessionEvent(
      makeEvent(
        { type: "AssistantMessageCompleted", message_id: "m3", content: "worker reply" },
        "agent_w1"
      ),
      ctx,
      agents
    );

    expect(ctx.projection.value.messages[0].role).toBe("worker");
    expect(ctx.projection.value.messages[0].sourceAgentId).toBe("agent_w1");
  });

  it("does not attribute AssistantMessageCompleted from agent_system", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent(
        { type: "AssistantMessageCompleted", message_id: "m4", content: "system reply" },
        "agent_system"
      ),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].role).toBe("assistant");
    expect(ctx.projection.value.messages[0].sourceAgentId).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — session lifecycle
// ---------------------------------------------------------------------------
describe("applySessionEvent — session lifecycle", () => {
  it("marks cancelled on SessionCancelled", () => {
    const projection = emptyProjection();
    projection.token_stream = "partial response";
    const ctx = makeCtx({
      projection: ref(projection) as Ref<SessionProjection>,
      isStreaming: ref(true)
    });
    applySessionEvent(
      makeEvent({ type: "SessionCancelled", reason: "user stopped" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.cancelled).toBe(true);
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.isStreaming.value).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — task events
// ---------------------------------------------------------------------------
describe("applySessionEvent — task events", () => {
  it("pushes task title on AgentTaskCreated", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "AgentTaskCreated",
        task_id: "t1",
        title: "do the thing",
        agent_id: "a1",
        role: "Worker"
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.task_titles).toEqual(["do the thing"]);
  });

  it("stops streaming on AgentTaskCompleted", () => {
    const ctx = makeCtx({ isStreaming: ref(true) });
    applySessionEvent(
      makeEvent({ type: "AgentTaskCompleted", task_id: "t1", agent_id: "a1" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.isStreaming.value).toBe(false);
  });

  it("pushes error message on AgentTaskFailed", () => {
    const ctx = makeCtx({ isStreaming: ref(true) });
    applySessionEvent(
      makeEvent({ type: "AgentTaskFailed", task_id: "t1", agent_id: "a1", error: "boom" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages).toHaveLength(1);
    expect(ctx.projection.value.messages[0].role).toBe("assistant");
    expect(ctx.projection.value.messages[0].content).toContain("boom");
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.isStreaming.value).toBe(false);
  });

  it("deduplicates a repeated AgentTaskFailed error already reported by the composer", () => {
    const ctx = makeCtx({ isStreaming: ref(true) });
    appendAssistantErrorMessage(ctx.projection.value, "boom");

    applySessionEvent(
      makeEvent({ type: "AgentTaskFailed", task_id: "t1", agent_id: "a1", error: "boom" }),
      ctx,
      makeAgentsStore()
    );

    expect(ctx.projection.value.messages).toEqual([{ role: "assistant", content: "[error] boom" }]);
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.isStreaming.value).toBe(false);
  });

  it("uses fallback text when AgentTaskFailed has no error", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({ type: "AgentTaskFailed", task_id: "t1", agent_id: "a1", error: "" }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].content).toContain("Unknown error");
  });

  it("projects TaskDecomposed as system message", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "TaskDecomposed",
        parent_task_id: "parent",
        sub_task_ids: ["a", "b"]
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].role).toBe("system");
    expect(ctx.projection.value.messages[0].content).toContain("2 sub-tasks");
  });

  it("projects TaskBlocked as system message", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "TaskBlocked",
        task_id: "t1",
        blocking_task_id: "t0",
        reason: "dependency failed"
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].role).toBe("system");
    expect(ctx.projection.value.messages[0].content).toContain("blocked");
  });

  it("uses fallback reason when TaskBlocked has no reason", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "TaskBlocked",
        task_id: "t1",
        blocking_task_id: "t0",
        reason: ""
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].content).toContain("dependency failed");
  });

  it("projects TaskRetried as system message with attempt number", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({ type: "TaskRetried", task_id: "t1", attempt: 3 }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.messages[0].role).toBe("system");
    expect(ctx.projection.value.messages[0].content).toContain("attempt 3");
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — context lifecycle
// ---------------------------------------------------------------------------
describe("applySessionEvent — context lifecycle", () => {
  it("captures ContextAssembled into lastContextUsage", () => {
    const ctx = makeCtx();
    const usage: ContextUsage = {
      total_tokens: 10_000,
      budget_tokens: 150_000,
      context_window: 200_000,
      output_reservation: 20_000,
      by_source: [["history", 10_000]],
      estimator: "cl100k_base",
      corrected_by_real_usage: false
    };
    applySessionEvent(makeEvent({ type: "ContextAssembled", usage }), ctx, makeAgentsStore());
    expect(ctx.lastContextUsage.value).toEqual(usage);
  });

  it("sets compacting=true and projects Running on ContextCompactionStarted", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionStarted",
        reason: { type: "UserRequested" },
        before_tokens: 180_000,
        candidate_event_count: 12
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.compaction).toEqual({ type: "Running" });
    expect(ctx.compacting.value).toBe(true);
    expect(ctx.lastCompactionError.value).toBeNull();
  });

  it("clears compacting and projects Completed on ContextCompactionCompleted", () => {
    const ctx = makeCtx({
      compacting: ref(true),
      lastCompactionError: ref("previous error")
    });
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionCompleted",
        summary_id: "sum_1",
        after_tokens: 30_000,
        fallback_used: false
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.compaction).toEqual({ type: "Completed" });
    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBeNull();
  });

  it("records error and projects Failed on ContextCompactionFailed", () => {
    const ctx = makeCtx({ compacting: ref(true) });
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionFailed",
        error: "timeout",
        fallback_used: false
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.compaction).toEqual({ type: "Failed", error: "timeout" });
    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBe("timeout");
  });

  it("keeps the failed status when fallback completion follows ContextCompactionFailed", () => {
    const ctx = makeCtx({ compacting: ref(true) });
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionFailed",
        error: "model timeout",
        fallback_used: true
      }),
      ctx,
      makeAgentsStore()
    );
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionCompleted",
        summary_id: "sum_1",
        after_tokens: 10_000,
        fallback_used: true
      }),
      ctx,
      makeAgentsStore()
    );

    expect(ctx.projection.value.compaction).toEqual({
      type: "Failed",
      error: "model timeout"
    });
    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBe("model timeout");
  });

  it("sets compaction Skipped status with reason and ratio", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "ContextCompactionSkipped",
        reason: { type: "AlreadyCompacting" },
        ratio: 0.42
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.projection.value.compaction).toEqual({
      type: "Skipped",
      reason: { type: "AlreadyCompacting" },
      ratio: 0.42
    });
    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — model profile switch
// ---------------------------------------------------------------------------
describe("applySessionEvent — ModelProfileSwitched", () => {
  it("updates profile, reasoning effort, and model limits", () => {
    const ctx = makeCtx();
    applySessionEvent(
      makeEvent({
        type: "ModelProfileSwitched",
        from_profile: "fast",
        to_profile: "opus",
        effective_at: "2026-05-06T00:00:00Z",
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry",
        reasoning_effort: "high"
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.currentProfile.value).toBe("opus");
    expect(ctx.currentReasoningEffort.value).toBe("high");
    expect(ctx.modelLimits.value).toEqual({
      context_window: 200_000,
      output_limit: 16_384,
      source: "builtin_registry"
    });
  });

  it("sets reasoning_effort to null when absent", () => {
    const ctx = makeCtx({ currentReasoningEffort: ref("high") as Ref<string | null> });
    applySessionEvent(
      makeEvent({
        type: "ModelProfileSwitched",
        from_profile: "opus",
        to_profile: "sonnet",
        effective_at: "2026-05-06T00:00:00Z",
        context_window: 200_000,
        output_limit: 8_192,
        limit_source: "user_config"
      }),
      ctx,
      makeAgentsStore()
    );
    expect(ctx.currentReasoningEffort.value).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// applySessionEvent — no-op events
// ---------------------------------------------------------------------------
describe("applySessionEvent — no-op events", () => {
  const noOpTypes = [
    "AgentSpawned",
    "AgentIdle",
    "SessionInitialized",
    "ModelRequestStarted",
    "ModelToolCallRequested",
    "ToolInvocationStarted",
    "ToolInvocationCompleted",
    "ToolInvocationFailed",
    "PermissionRequested",
    "PermissionGranted",
    "PermissionDenied",
    "FilePatchProposed",
    "FilePatchApplied",
    "MemoryProposed",
    "MemoryAccepted",
    "MemoryRejected",
    "ReviewerFindingAdded",
    "WorkspaceOpened"
  ];

  it.each(noOpTypes)("does not mutate projection for %s", (type) => {
    const ctx = makeCtx();
    const before = JSON.stringify(ctx.projection.value);
    applySessionEvent(makeEvent({ type } as never), ctx, makeAgentsStore());
    expect(JSON.stringify(ctx.projection.value)).toBe(before);
    expect(ctx.isStreaming.value).toBe(false);
  });

  it("AgentTaskStarted is a no-op", () => {
    const ctx = makeCtx();
    const before = JSON.stringify(ctx.projection.value);
    applySessionEvent(
      makeEvent({ type: "AgentTaskStarted", task_id: "t1", agent_id: "a1" }),
      ctx,
      makeAgentsStore()
    );
    expect(JSON.stringify(ctx.projection.value)).toBe(before);
  });
});

// ---------------------------------------------------------------------------
// setProjectionFromSnapshot
// ---------------------------------------------------------------------------
describe("setProjectionFromSnapshot", () => {
  it("hydrates projection, context usage, model limits, and compaction from snapshot", () => {
    const ctx = makeCtx();
    const taskGraphStore = makeTaskGraphStore();
    const usage: ContextUsage = {
      total_tokens: 50_000,
      budget_tokens: 180_000,
      context_window: 200_000,
      output_reservation: 20_000,
      by_source: [["history", 50_000]],
      estimator: "cl100k_base",
      corrected_by_real_usage: false
    };
    const snapshot: SessionProjection = {
      messages: [{ role: "user", content: "hi" }],
      task_titles: ["task 1"],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: usage,
      model_limits: { context_window: 200_000, output_limit: 8_192, source: "builtin_registry" },
      compaction: { type: "Running" }
    };

    setProjectionFromSnapshot(snapshot, ctx, taskGraphStore, "ses_1");

    expect(ctx.projection.value.messages).toHaveLength(1);
    expect(ctx.isStreaming.value).toBe(false);
    expect(ctx.lastContextUsage.value).toEqual(usage);
    expect(ctx.modelLimits.value).toEqual({
      context_window: 200_000,
      output_limit: 8_192,
      source: "builtin_registry"
    });
    expect(ctx.compacting.value).toBe(true);
    expect(ctx.lastCompactionError.value).toBeNull();
  });

  it("surfaces failed compaction error from snapshot", () => {
    const ctx = makeCtx();
    const snapshot: SessionProjection = {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: null,
      model_limits: null,
      compaction: { type: "Failed", error: "model timeout" }
    };

    setProjectionFromSnapshot(snapshot, ctx, makeTaskGraphStore(), null);

    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBe("model timeout");
  });

  it("defaults compaction to Idle when missing from snapshot", () => {
    const ctx = makeCtx();
    const snapshot = {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: null,
      model_limits: null
      // compaction field deliberately omitted
    } as unknown as SessionProjection;

    setProjectionFromSnapshot(snapshot, ctx, makeTaskGraphStore(), null);

    expect(ctx.projection.value.compaction).toEqual({ type: "Idle" });
    expect(ctx.compacting.value).toBe(false);
  });

  it("forwards task_graph to taskGraphStore", () => {
    const ctx = makeCtx();
    const taskGraphStore = makeTaskGraphStore();
    const tasks = [{ id: "t1", title: "a", state: "Completed" as const, children: [] }];
    const snapshot: SessionProjection = {
      messages: [],
      task_titles: [],
      task_graph: { tasks } as never,
      token_stream: "",
      cancelled: false,
      last_context_usage: null,
      model_limits: null,
      compaction: { type: "Idle" }
    };

    setProjectionFromSnapshot(snapshot, ctx, taskGraphStore, "ses_1");

    expect(taskGraphStore.setTaskGraph).toHaveBeenCalledWith(tasks, "ses_1");
  });
});

// ---------------------------------------------------------------------------
// resetProjectionState
// ---------------------------------------------------------------------------
describe("resetProjectionState", () => {
  it("resets all fields to their initial values", () => {
    const ctx = makeCtx({
      isStreaming: ref(true),
      lastSendError: ref("some error") as Ref<string | null>,
      compacting: ref(true),
      lastCompactionError: ref("compaction error") as Ref<string | null>,
      lastContextUsage: ref({
        total_tokens: 1000,
        budget_tokens: 100_000,
        context_window: 200_000,
        output_reservation: 20_000,
        by_source: [],
        estimator: "cl100k",
        corrected_by_real_usage: false
      }) as Ref<ContextUsage | null>,
      modelLimits: ref({
        context_window: 200_000,
        output_limit: 8_192,
        source: "builtin_registry"
      }) as Ref<ProjectedModelLimits | null>
    });
    // Pre-populate projection
    ctx.projection.value.messages.push({ role: "user", content: "hi" });
    ctx.projection.value.token_stream = "partial";

    const agentsStore = makeAgentsStore();
    const streamsByTask = ref(new Map([["t1", "stream"]])) as Ref<Map<string, string>>;

    resetProjectionState(ctx, agentsStore, streamsByTask);

    expect(ctx.projection.value.messages).toEqual([]);
    expect(ctx.projection.value.token_stream).toBe("");
    expect(ctx.projection.value.cancelled).toBe(false);
    expect(ctx.isStreaming.value).toBe(false);
    expect(ctx.lastSendError.value).toBeNull();
    expect(ctx.lastContextUsage.value).toBeNull();
    expect(ctx.modelLimits.value).toBeNull();
    expect(ctx.compacting.value).toBe(false);
    expect(ctx.lastCompactionError.value).toBeNull();
    expect(streamsByTask.value.size).toBe(0);
    expect(agentsStore.clearAgents).toHaveBeenCalled();
  });
});
