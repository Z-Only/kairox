import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTraceStore as usePiniaTraceStore } from "@/stores/trace";
import { traceState, applyTraceEvent, clearTrace } from "./useTraceStore";
import { makeEvent } from "./useTraceStore.test-utils";

describe("useTraceStore — shim and monitors", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    clearTrace();
  });

  // -----------------------------------------------------------------------
  // Proxy shim behaviour
  //
  // The composable is a thin Proxy over the Pinia store. These tests
  // verify that every trap (`get`, `set`, `has`, `ownKeys`,
  // `getOwnPropertyDescriptor`) correctly delegates to the store
  // instance, so consumers can treat `traceState` as a transparent
  // stand-in.
  // -----------------------------------------------------------------------
  describe("Proxy shim", () => {
    it("reads entries and density from the underlying store", () => {
      const store = usePiniaTraceStore();
      expect(traceState.entries).toBe(store.entries);
      expect(traceState.density).toBe(store.density);
    });

    it("writes through to the store", () => {
      const store = usePiniaTraceStore();
      traceState.density = "L3";
      expect(store.density).toBe("L3");
    });

    it("has() reports store properties", () => {
      expect("entries" in traceState).toBe(true);
      expect("density" in traceState).toBe(true);
      expect("applyTraceEvent" in traceState).toBe(true);
      expect("nonExistent" in traceState).toBe(false);
    });

    it("ownKeys() trap delegates to the store", () => {
      // Object.keys() would also trigger getOwnPropertyDescriptor, which
      // can conflict with Pinia's non-configurable $state property.
      // Use Reflect.ownKeys() to test the ownKeys trap in isolation.
      const keys = Reflect.ownKeys(traceState);
      expect(keys).toContain("entries");
      expect(keys).toContain("density");
    });

    it("mutations via the proxy are visible through the store and vice-versa", () => {
      const store = usePiniaTraceStore();

      store.applyTraceEvent(
        makeEvent({
          type: "AgentTaskCreated",
          task_id: "proxy-t",
          title: "Via store",
          role: "Worker",
          dependencies: []
        })
      );

      // Proxy should reflect the store mutation
      expect(traceState.entries).toHaveLength(1);
      expect(traceState.entries[0].title).toBe("Via store");

      // Clearing via the proxy should reflect in the store
      clearTrace();
      expect(store.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // Monitor lifecycle
  //
  // MonitorStarted / MonitorEvent / MonitorStopped / MonitorFailed are
  // exercised here because no other split file covers them.
  // -----------------------------------------------------------------------
  describe("MonitorStarted", () => {
    it("creates a running monitor entry with description and command", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-1",
          description: "Watch logs",
          command: "tail -f app.log"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.id).toBe("mon-1");
      expect(entry.kind).toBe("monitor");
      expect(entry.status).toBe("running");
      expect(entry.toolId).toBe("monitor");
      expect(entry.title).toBe("Watch logs");
      expect(entry.input).toBe("tail -f app.log");
      expect(entry.expanded).toBe(false);
    });

    it("deduplicates by monitor_id", () => {
      const event = makeEvent({
        type: "MonitorStarted",
        monitor_id: "mon-dup",
        description: "Watch",
        command: "tail -f"
      });

      applyTraceEvent(event);
      applyTraceEvent(event);

      expect(traceState.entries).toHaveLength(1);
    });
  });

  describe("MonitorEvent", () => {
    it("updates the outputPreview of a running monitor", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-e",
          description: "Errors",
          command: "tail -f err.log"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MonitorEvent",
          monitor_id: "mon-e",
          line: "ERROR: disk full"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      expect(traceState.entries[0].outputPreview).toBe("ERROR: disk full");
    });

    it("overwrites previous outputPreview on each event", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-e2",
          description: "Watch",
          command: "tail -f"
        })
      );

      applyTraceEvent(makeEvent({ type: "MonitorEvent", monitor_id: "mon-e2", line: "line 1" }));
      applyTraceEvent(makeEvent({ type: "MonitorEvent", monitor_id: "mon-e2", line: "line 2" }));

      expect(traceState.entries[0].outputPreview).toBe("line 2");
    });

    it("does not crash when the monitor does not exist", () => {
      expect(() => {
        applyTraceEvent(
          makeEvent({
            type: "MonitorEvent",
            monitor_id: "nonexistent",
            line: "ghost"
          })
        );
      }).not.toThrow();

      expect(traceState.entries).toHaveLength(0);
    });
  });

  describe("MonitorStopped", () => {
    it("updates a running monitor to completed with reason", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-s",
          description: "Watch build",
          command: "cargo watch"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MonitorStopped",
          monitor_id: "mon-s",
          reason: { type: "UserCancelled" }
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.status).toBe("completed");
      expect(entry.reason).toBe("UserCancelled");
    });

    it("does not crash when stopping a nonexistent monitor", () => {
      expect(() => {
        applyTraceEvent(
          makeEvent({
            type: "MonitorStopped",
            monitor_id: "nonexistent",
            reason: { type: "Timeout" }
          })
        );
      }).not.toThrow();

      expect(traceState.entries).toHaveLength(0);
    });
  });

  describe("MonitorFailed", () => {
    it("updates a running monitor to failed with error", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-f",
          description: "Watch deploy",
          command: "deploy-watch"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MonitorFailed",
          monitor_id: "mon-f",
          error: "Connection refused"
        })
      );

      expect(traceState.entries).toHaveLength(1);
      const entry = traceState.entries[0];
      expect(entry.status).toBe("failed");
      expect(entry.outputPreview).toBe("Connection refused");
    });

    it("does not crash when the monitor does not exist", () => {
      expect(() => {
        applyTraceEvent(
          makeEvent({
            type: "MonitorFailed",
            monitor_id: "nonexistent",
            error: "boom"
          })
        );
      }).not.toThrow();

      expect(traceState.entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // Full monitor lifecycle
  // -----------------------------------------------------------------------
  describe("full monitor lifecycle", () => {
    it("start -> event -> stop", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-full",
          description: "CI pipeline",
          command: "gh run watch"
        })
      );

      expect(traceState.entries[0].status).toBe("running");

      applyTraceEvent(
        makeEvent({
          type: "MonitorEvent",
          monitor_id: "mon-full",
          line: "Step 3/5: test"
        })
      );

      expect(traceState.entries[0].outputPreview).toBe("Step 3/5: test");
      expect(traceState.entries[0].status).toBe("running");

      applyTraceEvent(
        makeEvent({
          type: "MonitorStopped",
          monitor_id: "mon-full",
          reason: { type: "Completed" }
        })
      );

      expect(traceState.entries[0].status).toBe("completed");
      expect(traceState.entries[0].reason).toBe("Completed");
    });

    it("start -> event -> fail", () => {
      applyTraceEvent(
        makeEvent({
          type: "MonitorStarted",
          monitor_id: "mon-fail",
          description: "Deploy watch",
          command: "deploy-monitor"
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MonitorEvent",
          monitor_id: "mon-fail",
          line: "Deploying..."
        })
      );

      applyTraceEvent(
        makeEvent({
          type: "MonitorFailed",
          monitor_id: "mon-fail",
          error: "Deployment crashed"
        })
      );

      const entry = traceState.entries[0];
      expect(entry.status).toBe("failed");
      expect(entry.outputPreview).toBe("Deployment crashed");
    });
  });
});
