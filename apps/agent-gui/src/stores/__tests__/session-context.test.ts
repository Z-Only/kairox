// P3 Task 5 — verify useSessionStore exposes lastContextUsage / modelLimits /
// compacting / lastCompactionError reactive state and updates on the four
// context lifecycle events + setProjection / resetProjection paths.
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, beforeEach } from "vitest";
import { useSessionStore } from "@/stores/session";
import type { DomainEvent, EventPayload, ContextUsage, SessionProjection } from "@/types";

function makeUsage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  return {
    total_tokens: 12_000,
    budget_tokens: 180_000,
    context_window: 200_000,
    output_reservation: 20_000,
    by_source: [
      ["system", 2_000],
      ["history", 10_000]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false,
    ...overrides
  };
}

function makeEvent(payload: EventPayload): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_test",
    session_id: "ses_test",
    timestamp: new Date().toISOString(),
    source_agent_id: "agent_system",
    privacy: "minimal_trace",
    event_type: payload.type,
    payload
  } as DomainEvent;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("useSessionStore — context fields", () => {
  it("starts with null context usage and idle compaction", () => {
    const session = useSessionStore();
    expect(session.lastContextUsage).toBeNull();
    expect(session.modelLimits).toBeNull();
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();
  });

  it("captures ContextAssembled into lastContextUsage", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "ContextAssembled", usage: makeUsage() }));
    expect(session.lastContextUsage?.total_tokens).toBe(12_000);
    expect(session.lastContextUsage?.budget_tokens).toBe(180_000);
  });

  it("flips compacting on ContextCompactionStarted/Completed/Failed", () => {
    const session = useSessionStore();

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionStarted",
        reason: { type: "UserRequested" },
        before_tokens: 180_000,
        candidate_event_count: 12
      })
    );
    expect(session.compacting).toBe(true);
    expect(session.lastCompactionError).toBeNull();

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionCompleted",
        summary_id: "sum_1",
        after_tokens: 30_000,
        fallback_used: false
      })
    );
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionFailed",
        error: "model timeout",
        fallback_used: true
      })
    );
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBe("model timeout");
  });

  it("setProjection hydrates context refs from the snapshot", () => {
    const session = useSessionStore();
    session.setProjection({
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: makeUsage({ total_tokens: 50_000 }),
      model_limits: {
        context_window: 200_000,
        output_limit: 8_192,
        source: "builtin_registry"
      },
      compaction: { type: "Running" }
    } as unknown as SessionProjection);

    expect(session.lastContextUsage?.total_tokens).toBe(50_000);
    expect(session.modelLimits?.context_window).toBe(200_000);
    expect(session.compacting).toBe(true);
    expect(session.lastCompactionError).toBeNull();
  });

  it("setProjection surfaces the failed-compaction error", () => {
    const session = useSessionStore();
    session.setProjection({
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: null,
      model_limits: null,
      compaction: { type: "Failed", error: "model timeout" }
    } as unknown as SessionProjection);

    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBe("model timeout");
  });

  it("resetProjection clears the four context refs", () => {
    const session = useSessionStore();
    session.setProjection({
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: makeUsage(),
      model_limits: {
        context_window: 200_000,
        output_limit: 20_000,
        source: "user_config"
      },
      compaction: { type: "Running" }
    } as unknown as SessionProjection);

    session.resetProjection();

    expect(session.lastContextUsage).toBeNull();
    expect(session.modelLimits).toBeNull();
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();
  });

  it("updates currentProfile and modelLimits on ModelProfileSwitched", () => {
    const session = useSessionStore();
    // Sanity: store starts with the default "fast" profile (verified at
    // `stores/session.ts:39` — `currentProfile = ref<string>("fast")`).
    expect(session.currentProfile).toBe("fast");
    expect(session.modelLimits).toBeNull();

    session.applyEvent(
      makeEvent({
        type: "ModelProfileSwitched",
        from_profile: "fast",
        to_profile: "opus",
        effective_at: "2026-05-09T10:00:00Z",
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry"
      })
    );

    expect(session.currentProfile).toBe("opus");
    expect(session.modelLimits).toEqual({
      context_window: 200_000,
      output_limit: 16_384,
      source: "builtin_registry"
    });
  });
});
