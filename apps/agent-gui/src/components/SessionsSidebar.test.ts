import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import SessionsSidebar from "./SessionsSidebar.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));
vi.mock("../stores/taskGraph", () => ({
  taskGraphState: { tasks: [], currentSessionId: null, loading: false },
  clearTaskGraph: vi.fn(),
  setTaskGraph: vi.fn()
}));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { sessionState, resetProjection } from "../stores/session";

beforeEach(() => {
  sessionState.sessions = [];
  sessionState.currentSessionId = null;
  sessionState.currentProfile = "fast";
  resetProjection();
  vi.clearAllMocks();
});

describe("SessionsSidebar", () => {
  it("renders session titles", () => {
    sessionState.sessions = [
      { id: "s1", title: "Chat about Rust", profile: "fast" } as never,
      { id: "s2", title: "Debug session", profile: "slow" } as never
    ];
    const wrapper = mount(SessionsSidebar);
    expect(wrapper.text()).toContain("Chat about Rust");
    expect(wrapper.text()).toContain("Debug session");
  });

  it("shows empty hint when no sessions", () => {
    const wrapper = mount(SessionsSidebar);
    expect(wrapper.text()).toContain("No sessions yet");
  });

  it("invokes switch_session on session click", async () => {
    sessionState.sessions = [
      { id: "s1", title: "Session 1", profile: "fast" } as never
    ];
    mockedInvoke.mockResolvedValueOnce({
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mount(SessionsSidebar);
    await wrapper.find(".session-item").trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("switch_session", {
      sessionId: "s1"
    });
  });

  it("opens new session dialog on + New click", async () => {
    mockedInvoke.mockResolvedValueOnce([
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o",
        local: false,
        has_api_key: true
      }
    ]);
    const wrapper = mount(SessionsSidebar);
    await wrapper.find(".new-session-btn").trigger("click");
    expect(wrapper.text()).toContain("New Session");
  });
});
