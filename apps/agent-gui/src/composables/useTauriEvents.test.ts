import { describe, it, expect, beforeEach, vi, type Mock } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

// Store the callback so we can invoke it in tests.
let listenCallback: ((event: any) => void) | null = null;
const unlistenFn = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, callback: (event: any) => void) => {
    listenCallback = callback;
    return Promise.resolve(unlistenFn);
  })
}));

vi.mock("@/composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn()
}));

import { listen } from "@tauri-apps/api/event";
import { useTauriEvents } from "./useTauriEvents";
import { useSessionStore } from "@/stores/session";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useUiStore } from "@/stores/ui";
import { useMcpStore } from "@/stores/mcp";
import { useAgentsStore } from "@/stores/agents";
import { useCatalogStore } from "@/stores/catalog";
import { useMemoryStore } from "@/stores/memory";
import { applyTraceEvent } from "@/composables/useTraceStore";

const Dummy = defineComponent({
  setup() {
    useTauriEvents();
    return () => null;
  }
});

describe("useTauriEvents", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    listenCallback = null;
  });

  it("subscribes to session-event and marks session as connected on success", async () => {
    const wrapper = mount(Dummy);

    // Flush microtasks for the listen promise resolution
    await Promise.resolve();
    await Promise.resolve();

    expect(listen).toHaveBeenCalledWith("session-event", expect.any(Function));
    const session = useSessionStore();
    expect(session.connected).toBe(true);

    wrapper.unmount();
  });

  it("calls unlisten when component is unmounted", async () => {
    const wrapper = mount(Dummy);

    await Promise.resolve();
    await Promise.resolve();

    wrapper.unmount();

    // tryOnScopeDispose fires, which calls unlistenFn
    await Promise.resolve();
    expect(unlistenFn).toHaveBeenCalled();
  });

  it("sets session.connected to false on unmount", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    expect(session.connected).toBe(true);

    wrapper.unmount();
    await Promise.resolve();

    expect(session.connected).toBe(false);
  });

  it("routes session-scoped events to session.applyEvent and taskGraph.applyTaskEvent", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    const taskGraph = useTaskGraphStore();

    // Set current session so events are accepted
    session.currentSessionId = "sess-1";

    const applyEventSpy = vi.spyOn(session, "applyEvent").mockImplementation(() => {});
    const applyTaskEventSpy = vi.spyOn(taskGraph, "applyTaskEvent").mockImplementation(() => {});

    const domainEvent = {
      session_id: "sess-1",
      payload: {
        type: "AgentTaskStarted",
        task_id: "T1"
      }
    };

    listenCallback!({ payload: domainEvent });

    expect(applyEventSpy).toHaveBeenCalledWith(domainEvent);
    expect(applyTaskEventSpy).toHaveBeenCalledWith(domainEvent.payload);
    expect(applyTraceEvent).toHaveBeenCalledWith(domainEvent);

    wrapper.unmount();
  });

  it("pushes error notification when AgentTaskFailed has an error", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    const ui = useUiStore();
    session.currentSessionId = "sess-1";

    vi.spyOn(session, "applyEvent").mockImplementation(() => {});

    const domainEvent = {
      session_id: "sess-1",
      payload: {
        type: "AgentTaskFailed",
        task_id: "T1",
        error: "Out of memory"
      }
    };

    listenCallback!({ payload: domainEvent });

    const errorNotice = ui.notifications.find(
      (n) => n.level === "error" && n.message === "Out of memory"
    );
    expect(errorNotice).toBeDefined();

    wrapper.unmount();
  });

  it("routes agent lifecycle events to the agents store", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    const agents = useAgentsStore();
    session.currentSessionId = "sess-1";

    vi.spyOn(session, "applyEvent").mockImplementation(() => {});
    const applyAgentSpy = vi.spyOn(agents, "applyAgentEvent").mockImplementation(() => {});

    const domainEvent = {
      session_id: "sess-1",
      payload: {
        type: "AgentTaskStarted",
        task_id: "T1"
      }
    };

    listenCallback!({ payload: domainEvent });

    expect(applyAgentSpy).toHaveBeenCalledWith(domainEvent.payload);

    wrapper.unmount();
  });

  it("refreshes memory browser data when memory lifecycle events arrive", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    const memory = useMemoryStore();
    session.currentSessionId = "sess-1";

    vi.spyOn(session, "applyEvent").mockImplementation(() => {});
    const loadMemoriesSpy = vi.spyOn(memory, "loadMemories").mockResolvedValue();

    for (const payload of [
      {
        type: "MemoryProposed",
        memory_id: "mem-1",
        scope: "user",
        key: "lang",
        content: "Rust"
      },
      {
        type: "MemoryAccepted",
        memory_id: "mem-1",
        scope: "user",
        key: "lang",
        content: "Rust"
      },
      {
        type: "MemoryRejected",
        memory_id: "mem-1",
        reason: "User rejected"
      }
    ]) {
      listenCallback!({
        payload: {
          session_id: "sess-1",
          payload
        }
      });
    }

    expect(loadMemoriesSpy).toHaveBeenCalledTimes(3);

    wrapper.unmount();
  });

  it("ignores session-scoped events for a different session", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const session = useSessionStore();
    const taskGraph = useTaskGraphStore();
    session.currentSessionId = "sess-1";

    const applyEventSpy = vi.spyOn(session, "applyEvent").mockImplementation(() => {});
    const applyTaskEventSpy = vi.spyOn(taskGraph, "applyTaskEvent").mockImplementation(() => {});

    const domainEvent = {
      session_id: "sess-OTHER",
      payload: {
        type: "AgentTaskStarted",
        task_id: "T1"
      }
    };

    listenCallback!({ payload: domainEvent });

    expect(applyEventSpy).not.toHaveBeenCalled();
    expect(applyTaskEventSpy).not.toHaveBeenCalled();

    wrapper.unmount();
  });

  it("routes MCP events to the mcp store regardless of session", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const mcp = useMcpStore();
    const handleMcpSpy = vi.spyOn(mcp, "handleMcpEvent").mockImplementation(() => {});

    const mcpEvents = [
      "McpServerStarting",
      "McpServerReady",
      "McpServerStopped",
      "McpServerFailed",
      "McpToolCallStarted",
      "McpToolCallCompleted",
      "McpTrustGranted",
      "McpTrustRevoked"
    ];

    for (const eventType of mcpEvents) {
      handleMcpSpy.mockClear();
      const domainEvent = {
        payload: { type: eventType, server_id: "test-server" }
      };
      listenCallback!({ payload: domainEvent });
      expect(handleMcpSpy).toHaveBeenCalledWith(domainEvent.payload);
    }

    wrapper.unmount();
  });

  it("routes CatalogSourceAdded to catalog.fetchSources", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const catalog = useCatalogStore();
    const fetchSourcesSpy = vi.spyOn(catalog, "fetchSources").mockResolvedValue();

    const domainEvent = {
      payload: { type: "CatalogSourceAdded", source: "new-src" }
    };

    listenCallback!({ payload: domainEvent });

    expect(fetchSourcesSpy).toHaveBeenCalled();

    wrapper.unmount();
  });

  it("routes CatalogSourceFailed to catalog.handleSourceFailed", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const catalog = useCatalogStore();
    const handleFailedSpy = vi.spyOn(catalog, "handleSourceFailed");

    const domainEvent = {
      payload: { type: "CatalogSourceFailed", source: "registry-a", error: "timeout" }
    };

    listenCallback!({ payload: domainEvent });

    expect(handleFailedSpy).toHaveBeenCalledWith("registry-a", "timeout");

    wrapper.unmount();
  });

  it("routes CatalogSourceResultsArrived to catalog.mergeSourceResults", async () => {
    const wrapper = mount(Dummy);
    await Promise.resolve();
    await Promise.resolve();

    const catalog = useCatalogStore();
    const mergeSpy = vi.spyOn(catalog, "mergeSourceResults").mockImplementation(() => {});

    const entries = [{ id: "entry-1", source: "registry-a" }];
    const domainEvent = {
      payload: { type: "CatalogSourceResultsArrived", source: "registry-a", entries }
    };

    listenCallback!({ payload: domainEvent });

    expect(mergeSpy).toHaveBeenCalledWith("registry-a", entries);

    wrapper.unmount();
  });
});

describe("useTauriEvents — listen failure", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("surfaces a listen() rejection as an error notification", async () => {
    // Override the listen mock to reject
    (listen as Mock).mockImplementationOnce(() => Promise.reject(new Error("channel closed")));

    const wrapper = mount(Dummy);

    // Flush microtasks so the rejected unlistenPromise reaches the .catch handler.
    await Promise.resolve();
    await Promise.resolve();

    const ui = useUiStore();
    const errorNotice = ui.notifications.find(
      (n) => n.level === "error" && n.message.startsWith("Failed to subscribe to session events")
    );

    expect(errorNotice).toBeDefined();
    expect(errorNotice!.message).toContain("channel closed");

    wrapper.unmount();
  });
});
