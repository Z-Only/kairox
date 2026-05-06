import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  setActivePinia(createPinia());
  clearTrace();
  useTaskGraphStore().clearTaskGraph();
  // MemoryBrowser (rendered when the Memory tab is activated) calls
  // `invoke('query_memories', ...)` on mount and assigns the result to
  // `memories.value`. Without a default resolved value, vitest mocks
  // return `undefined`, which makes `memories.length` throw inside the
  // template render. Supply a stable empty-array default so any invoke
  // call this test file does not override stays well-typed.
  mockedInvoke.mockResolvedValue([]);
});

describe("TraceTimeline", () => {
  it("shows Trace tab as active by default", () => {
    const wrapper = mount(TraceTimeline);
    const buttons = wrapper.findAll(".tab-group button");
    expect(buttons[0].classes()).toContain("active");
    expect(buttons[0].text()).toBe("Trace");
  });

  it("switches to Tasks tab when clicked", async () => {
    const wrapper = mount(TraceTimeline);
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[1].trigger("click");
    expect(buttons[1].classes()).toContain("active");
  });

  it("switches to Memory tab when clicked", async () => {
    const wrapper = mount(TraceTimeline);
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[2].trigger("click");
    expect(buttons[2].classes()).toContain("active");
  });

  it("cycles density when density buttons are clicked", async () => {
    const wrapper = mount(TraceTimeline);
    expect(traceState.density).toBe("L2");
    const densityButtons = wrapper.findAll(".density-toggles button");
    await densityButtons[2].trigger("click");
    expect(traceState.density).toBe("L3");
    await densityButtons[0].trigger("click");
    expect(traceState.density).toBe("L1");
  });
});
