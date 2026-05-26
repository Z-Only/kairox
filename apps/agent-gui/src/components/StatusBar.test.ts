import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import StatusBar from "./StatusBar.vue";
import { mountWithPlugins } from "@/test-utils/mount";
import { useSessionStore } from "@/stores/session";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

// StatusBar uses `useI18n()` (Task 7a NIT #6 — hardcoded strings →
// `t(...)` lookups), so the bare `mount()` no longer suffices. Use the
// shared helper that wires Pinia + i18n + the production router so the
// component can render under test.
function mountStatusBar(reusePinia = false) {
  const result = mountWithPlugins(StatusBar, { reusePinia });
  return result.wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  setActivePinia(createPinia());
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "get_profile_info") return [];
    return undefined;
  });
});

describe("StatusBar", () => {
  it("calls get_profile_info on mount", () => {
    mountStatusBar();
    expect(mockedInvoke).toHaveBeenCalledWith("get_profile_info");
  });

  it("displays the permission mode from the session store", async () => {
    const session = useSessionStore();
    session.permissionMode = "suggest";
    const wrapper = mountStatusBar(true);
    await flushPromises();
    expect(wrapper.text()).toContain("suggest");
  });

  it("renders sessions count, streaming and connected status as text", async () => {
    const wrapper = mountStatusBar(true);
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("Sessions");
    expect(text).toContain("Streaming");
    expect(text).toContain("Connected");
  });

  it("renders approval and sandbox status items with mapped labels", async () => {
    const session = useSessionStore();
    session.approvalPolicy = "on_request";
    session.sandboxPolicy = '{"kind":"workspace_write","network_access":false,"writable_roots":[]}';
    const wrapper = mountStatusBar(true);
    await flushPromises();

    const approval = wrapper.find('[data-test="status-bar-approval"]');
    const sandbox = wrapper.find('[data-test="status-bar-sandbox"]');
    expect(approval.exists()).toBe(true);
    expect(sandbox.exists()).toBe(true);
    expect(approval.text()).toContain("Approval");
    expect(approval.text()).toContain("On Request");
    expect(sandbox.text()).toContain("Sandbox");
    expect(sandbox.text()).toContain("Workspace Write");
  });

  it("updates approval and sandbox displays when policies change", async () => {
    const session = useSessionStore();
    session.approvalPolicy = "never";
    session.sandboxPolicy = '{"kind":"read_only"}';
    const wrapper = mountStatusBar(true);
    await flushPromises();

    expect(wrapper.find('[data-test="status-bar-approval"]').text()).toContain("Never");
    expect(wrapper.find('[data-test="status-bar-sandbox"]').text()).toContain("Read Only");

    session.approvalPolicy = "always";
    session.sandboxPolicy = '{"kind":"danger_full_access"}';
    await flushPromises();

    expect(wrapper.find('[data-test="status-bar-approval"]').text()).toContain("Always");
    expect(wrapper.find('[data-test="status-bar-sandbox"]').text()).toContain("Danger Full Access");
  });

  it("falls back to raw sandbox policy when JSON is invalid", async () => {
    const session = useSessionStore();
    session.sandboxPolicy = "not-json";
    const wrapper = mountStatusBar(true);
    await flushPromises();

    expect(wrapper.find('[data-test="status-bar-sandbox"]').text()).toContain("not-json");
  });
});
