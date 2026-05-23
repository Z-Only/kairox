import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { traceState, applyTraceEvent, clearTrace } from "./useTraceStore";
import { makeEvent } from "./useTraceStore.test-utils";

describe("useTraceStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    clearTrace();
  });

  // -----------------------------------------------------------------------
  // 3. ContextAssembled
  // -----------------------------------------------------------------------
  describe("ContextAssembled", () => {
    it("creates a completed entry with token usage and per-source breakdown as outputPreview", () => {
      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          usage: {
            total_tokens: 1500,
            budget_tokens: 100_000,
            context_window: 128_000,
            output_reservation: 28_000,
            by_source: [
              ["selected_file", 800],
              ["selected_file", 700]
            ],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("completed");
      expect(entry.toolId).toBe("context");
      expect(entry.title).toBe("Context assembled (1500 / 100000 tokens)");
      expect(entry.outputPreview).toBe("selected_file:800, selected_file:700");
    });

    it("generates unique IDs for multiple ContextAssembled events", async () => {
      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          usage: {
            total_tokens: 100,
            budget_tokens: 100_000,
            context_window: 128_000,
            output_reservation: 28_000,
            by_source: [],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
        })
      );

      // Small delay to ensure different Date.now()
      await new Promise((r) => setTimeout(r, 2));

      applyTraceEvent(
        makeEvent({
          type: "ContextAssembled",
          usage: {
            total_tokens: 200,
            budget_tokens: 100_000,
            context_window: 128_000,
            output_reservation: 28_000,
            by_source: [],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
        })
      );

      expect(traceState.entries).toHaveLength(2);
      expect(traceState.entries[0].id).not.toBe(traceState.entries[1].id);
    });
  });

  // -----------------------------------------------------------------------
  // 4. ModelRequestStarted
  // -----------------------------------------------------------------------
  describe("ModelRequestStarted", () => {
    it("creates a running entry with model info", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelRequestStarted",
          model_profile: "gpt-4",
          model_id: "gpt-4-0613"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.kind).toBe("tool");
      expect(entry.status).toBe("running");
      expect(entry.toolId).toBe("model");
      expect(entry.title).toBe("Model: gpt-4 / gpt-4-0613");
    });
  });

  // -----------------------------------------------------------------------
  // 5. ModelTokenDelta
  // -----------------------------------------------------------------------
  describe("ModelTokenDelta", () => {
    it("is skipped — no entry created", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelTokenDelta",
          delta: "Hello"
        })
      );

      expect(traceState.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // 6. AssistantMessageCompleted
  // -----------------------------------------------------------------------
  describe("AssistantMessageCompleted", () => {
    it("updates running model entry to completed", () => {
      applyTraceEvent(
        makeEvent({
          type: "ModelRequestStarted",
          model_profile: "gpt-4",
          model_id: "gpt-4-0613"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-1",
          content: "Here is the answer"
        })
      );

      // Model entry should be updated, no new assistant entry created
      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.status).toBe("completed");
      expect(entry.outputPreview).toBe("Here is the answer");
      expect(entry.durationMs).toBeGreaterThanOrEqual(0);
    });

    it("creates assistant entry when no running model exists", () => {
      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-2",
          content: "Standalone response"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("asst-2");
      expect(entry.toolId).toBe("assistant");
      expect(entry.status).toBe("completed");
      expect(entry.title).toBe("Assistant response");
      expect(entry.outputPreview).toBe("Standalone response");
    });

    it("deduplicates assistant entry by message_id", () => {
      // No running model, so assistant entry is created
      const event = makeEvent({
        type: "AssistantMessageCompleted",
        message_id: "asst-dup",
        content: "Response"
      });

      applyTraceEvent(event);
      applyTraceEvent(event);

      expect(traceState.entries).toHaveLength(1);
    });

    it("truncates outputPreview at 200 chars", () => {
      const longContent = "x".repeat(201);
      applyTraceEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "asst-long",
          content: longContent
        })
      );

      const entry = traceState.entries[0];
      expect(entry.outputPreview).toBe("x".repeat(200));
    });
  });
});
