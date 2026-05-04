import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { clearTaskGraph } from "../stores/taskGraph";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));
vi.mock("../stores/session", () => ({
  sessionState: {
    sessions: [],
    currentSessionId: null,
    workspaceId: null,
    currentProfile: "fast",
    isStreaming: false,
    connected: false,
    initialized: false,
    projection: {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    }
  },
  applyEvent: vi.fn(),
  setProjection: vi.fn(),
  resetProjection: vi.fn(),
  reportSendError: vi.fn()
}));
vi.mock("../stores/memory", () => ({
  memoryState: {
    memories: [],
    loading: false,
    filter: "all" as const,
    searchQuery: ""
  },
  loadMemories: vi.fn(),
  deleteMemoryItem: vi.fn(),
  setMemoryFilter: vi.fn()
}));

beforeEach(() => {
  clearTrace();
  clearTaskGraph();
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
