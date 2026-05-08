import { describe, it, expect, beforeEach, vi } from "vitest";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

// MemoryBrowser (rendered when the Memory tab is activated) calls
// `useI18n()` and `useConfirm()`, so any render path that mounts it
// requires the i18n plugin and the confirmDialog injection.
// `mountWithPlugins` wires i18n plus a fresh Pinia; we provide the
// confirm injection via `mount.global.provide`.
function mountTimeline() {
  return mountWithPlugins(TraceTimeline, {
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        }
      }
    }
  }).wrapper;
}

beforeEach(() => {
  clearTrace();
  // MemoryBrowser calls `invoke('query_memories', ...)` on mount and
  // assigns the result to `memories.value`. Without a default resolved
  // value, vitest mocks return `undefined`, which makes `memories.length`
  // throw inside the template render. Supply a stable empty-array
  // default so any invoke call this test file does not override stays
  // well-typed.
  mockedInvoke.mockResolvedValue([]);
  // The Pinia-bound `useTaskGraphStore().clearTaskGraph()` reset is now
  // done *after* mount inside each test (see `mountTimeline()` helper
  // calls below); the shared helper installs a fresh Pinia per mount so
  // there is no module-level state to reset before mount.
});

describe("TraceTimeline", () => {
  it("shows Trace tab as active by default", () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    expect(buttons[0].classes()).toContain("active");
    expect(buttons[0].text()).toBe("Trace");
  });

  it("switches to Tasks tab when clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[1].trigger("click");
    expect(buttons[1].classes()).toContain("active");
  });

  it("switches to Memory tab when clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[2].trigger("click");
    expect(buttons[2].classes()).toContain("active");
  });

  it("cycles density when density buttons are clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    expect(traceState.density).toBe("L2");
    const densityButtons = wrapper.findAll(".density-toolbar .density-btn");
    await densityButtons[2].trigger("click");
    expect(traceState.density).toBe("L3");
    await densityButtons[0].trigger("click");
    expect(traceState.density).toBe("L1");
  });
});
