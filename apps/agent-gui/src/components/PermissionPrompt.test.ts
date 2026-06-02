import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import PermissionPrompt from "./PermissionPrompt.vue";
import type { TraceEntryData } from "../types/trace";
import { mountWithPlugins } from "@/test-utils/mount";

// `PermissionPrompt.vue` calls `useI18n()`; bare `mount()` throws
// "Need to install with `app.use` function". `mountWithPlugins` installs
// i18n + router; `reusePinia: true` keeps the `beforeEach` pinia (and the
// `mcp.trustServer = mockedTrustServer` / `mcp.trustedServerIds` mutations
// each test sets up before mounting).
//
// Note: passing the extended-options shape (`{ mount, reusePinia }`) makes
// `mountWithPlugins` return `{ wrapper, router }` rather than a bare
// wrapper, so we unwrap `.wrapper` here to keep the call-site API
// identical to the previous `mount(...)` usage.
const mount = (comp: typeof PermissionPrompt, options: { props: { entry: TraceEntryData } }) =>
  mountWithPlugins(comp, { mount: options, reusePinia: true }).wrapper;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useMcpStore } from "@/stores/mcp";

const mockedTrustServer = vi.fn(() => Promise.resolve());

const permissionEntry: TraceEntryData = {
  id: "perm_1",
  kind: "permission",
  status: "pending",
  toolId: "shell_exec",
  title: "Run command: ls",
  startedAt: Date.now(),
  expanded: true
};

const memoryEntry: TraceEntryData = {
  id: "mem_1",
  kind: "memory",
  status: "pending",
  toolId: "memory.store",
  title: "Save user memory",
  startedAt: Date.now(),
  expanded: true,
  scope: "user",
  content: "Prefers Rust"
};

const mcpEntry: TraceEntryData = {
  id: "perm_mcp_1",
  kind: "permission",
  status: "pending",
  toolId: "mcp.github.list_repos",
  title: "MCP tool: list_repos",
  startedAt: Date.now(),
  expanded: true
};

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  // Replace store action with spy so we can assert invocation.
  const mcp = useMcpStore();
  mcp.trustServer = mockedTrustServer;
  mcp.trustedServerIds = [];
});

describe("PermissionPrompt", () => {
  it("displays tool_id and title for permission entries", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: permissionEntry }
    });
    expect(wrapper.text()).toContain("Permission Required");
    expect(wrapper.text()).toContain("shell_exec");
    expect(wrapper.text()).toContain("Run command: ls");
  });

  it("displays memory-specific labels for memory entries", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: memoryEntry }
    });
    expect(wrapper.text()).toContain("Memory Proposed");
    expect(wrapper.text()).toContain("Accept");
    expect(wrapper.text()).toContain("Reject");
  });

  it("invokes resolve_permission with grant on Allow click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: permissionEntry }
    });
    await wrapper.find('[data-test="permission-allow"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_1",
      decision: "grant"
    });
  });

  it("invokes resolve_permission with deny on Deny click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: permissionEntry }
    });
    await wrapper.find('[data-test="permission-deny"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_1",
      decision: "deny"
    });
  });

  it("invokes accept_memory on memory Accept click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: memoryEntry }
    });
    await wrapper.find('[data-test="permission-allow"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("accept_memory", {
      id: "mem_1"
    });
  });

  it("invokes reject_memory on memory Reject click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: memoryEntry }
    });
    await wrapper.find('[data-test="permission-deny"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("reject_memory", {
      id: "mem_1"
    });
  });
});

describe("PermissionPrompt MCP trust UI", () => {
  it("shows MCP server info for MCP tool IDs", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: mcpEntry }
    });
    expect(wrapper.text()).toContain("MCP Server:");
    expect(wrapper.text()).toContain("github");
    expect(wrapper.find(".mcp-permission-info").exists()).toBe(true);
  });

  it("does not show MCP info for non-MCP tool IDs", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: permissionEntry }
    });
    expect(wrapper.find(".mcp-permission-info").exists()).toBe(false);
  });

  it("shows trust checkbox when server is not trusted", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: mcpEntry }
    });
    expect(wrapper.find(".mcp-trust-check").exists()).toBe(true);
    expect(wrapper.find(".mcp-trusted-badge").exists()).toBe(false);
  });

  it("shows trusted badge when server is already trusted", () => {
    const mcp = useMcpStore();
    mcp.trustedServerIds = ["github"];
    const wrapper = mount(PermissionPrompt, {
      props: { entry: mcpEntry }
    });
    expect(wrapper.find(".mcp-trusted-badge").exists()).toBe(true);
    expect(wrapper.find(".mcp-trust-check").exists()).toBe(false);
  });

  it("calls trustServer when allowing with trust checkbox checked", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    mockedTrustServer.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: mcpEntry }
    });
    // The NCheckbox has been replaced with a native <input type="checkbox">.
    // Drive it via setValue which triggers the "change" event on the
    // underlying input element.
    const checkbox = wrapper.find<HTMLInputElement>('input[data-test="trust-server-checkbox"]');
    expect(checkbox.exists()).toBe(true);
    await checkbox.setValue(true);
    await wrapper.find('[data-test="permission-allow"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_mcp_1",
      decision: "grant"
    });
    expect(mockedTrustServer).toHaveBeenCalledWith("github");
  });

  it("does not call trustServer when trust checkbox is not checked", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(PermissionPrompt, {
      props: { entry: mcpEntry }
    });
    await wrapper.find('[data-test="permission-allow"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_mcp_1",
      decision: "grant"
    });
    expect(mockedTrustServer).not.toHaveBeenCalled();
  });

  it("audit anchors: exposes stable permission pilot selectors", () => {
    const wrapper = mount(PermissionPrompt, {
      props: { entry: permissionEntry }
    });

    expect(wrapper.find('[data-test="permission-prompt"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="permission-allow"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="permission-deny"]').exists()).toBe(true);
  });
});
