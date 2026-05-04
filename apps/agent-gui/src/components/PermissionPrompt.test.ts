import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import PermissionPrompt from "./PermissionPrompt.vue";
import type { TraceEntryData } from "../types/trace";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

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
    await wrapper.find(".btn-allow").trigger("click");
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
    await wrapper.find(".btn-deny").trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_1",
      decision: "deny"
    });
  });
});
