import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import TraceEntry from "./TraceEntry.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import type { TraceEntryData } from "../types/trace";

const baseEntry: TraceEntryData = {
  id: "entry-1",
  kind: "tool",
  status: "completed",
  toolId: "shell_exec",
  title: "List files",
  startedAt: Date.now(),
  expanded: false
};

beforeEach(() => {
  clearTrace();
});

describe("TraceEntry", () => {
  it("hides detail when expanded is false and density is L2", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, expanded: false }, density: "L2" }
    });
    expect(wrapper.find(".entry-detail").exists()).toBe(false);
  });

  it("shows detail when expanded is true and density is L2", () => {
    traceState.entries.push({ ...baseEntry, expanded: true, input: "ls -la" });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L2" }
    });
    expect(wrapper.find(".entry-detail").exists()).toBe(true);
    expect(wrapper.find(".entry-detail").text()).toContain("ls -la");
  });

  it("toggles expanded on row click", async () => {
    traceState.entries.push({ ...baseEntry });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L2" }
    });
    expect(traceState.entries[0].expanded).toBe(false);
    await wrapper.find(".entry-row").trigger("click");
    expect(traceState.entries[0].expanded).toBe(true);
  });

  it("shows correct status icon for running", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "running" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("⏳");
  });

  it("shows correct status icon for completed", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "completed" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("✅");
  });

  it("shows correct status icon for failed", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "failed" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("❌");
  });

  it("shows duration in seconds when durationMs is present", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, durationMs: 2500 }, density: "L2" }
    });
    expect(wrapper.find(".entry-duration").text()).toBe("2.5s");
  });

  it("applies kind CSS class for memory entries", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, kind: "memory" }, density: "L2" }
    });
    expect(wrapper.find(".trace-entry").classes()).toContain(
      "trace-entry--memory"
    );
  });
});
