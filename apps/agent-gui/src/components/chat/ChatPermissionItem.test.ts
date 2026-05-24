import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatPermissionItem from "@/components/chat/ChatPermissionItem.vue";

// ChatPermissionItem delegates rendering to PermissionPrompt, which
// imports `@tauri-apps/api/core`'s `invoke` (and `@tauri-apps/api/event`
// indirectly through the auto-registered UI-kit components). Mock both
// so the spec runs outside a Tauri host. We don't drive allow/deny
// here — interaction semantics are covered in PermissionPrompt's own
// test suite.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function mountItem(props: Record<string, unknown>) {
  return mountWithPlugins(ChatPermissionItem, {
    mount: { props },
    reusePinia: true
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("ChatPermissionItem", () => {
  it("renders with data-variant='tool' when variant=tool", () => {
    const wrapper = mountItem({
      id: "req_1",
      variant: "tool",
      toolId: "shell_exec",
      title: "Run command"
    });
    const root = wrapper.find('[data-test="chat-permission-item"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-variant")).toBe("tool");
  });

  it("renders with data-variant='memory' when variant=memory", () => {
    const wrapper = mountItem({
      id: "mem_1",
      variant: "memory",
      toolId: "memory.store",
      title: "Remember preference"
    });
    const root = wrapper.find('[data-test="chat-permission-item"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-variant")).toBe("memory");
  });

  it("passes through title / toolId / scope so they appear in the PermissionPrompt body", () => {
    const wrapper = mountItem({
      id: "req_2",
      variant: "tool",
      toolId: "fs.read",
      title: "Read /etc/passwd",
      scope: "session"
    });
    const text = wrapper.text();
    expect(text).toContain("Read /etc/passwd");
    expect(text).toContain("fs.read");
    expect(text).toContain("session");
  });

  it("renders the underlying PermissionPrompt allow/deny controls", () => {
    const wrapper = mountItem({
      id: "req_3",
      variant: "tool",
      toolId: "shell_exec",
      title: "Run command"
    });
    expect(wrapper.find('[data-test="permission-prompt"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="permission-allow"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="permission-deny"]').exists()).toBe(true);
  });
});
