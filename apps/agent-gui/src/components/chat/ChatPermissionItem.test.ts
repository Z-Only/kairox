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

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

function mountItem(props: Record<string, unknown>) {
  return mountWithPlugins(ChatPermissionItem, {
    mount: { props, attachTo: document.body },
    reusePinia: true
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
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

  it("does not render tool shortcut hints for memory prompts", () => {
    const wrapper = mountItem({
      id: "mem_2",
      variant: "memory",
      toolId: "memory.store",
      title: "Save user memory",
      scope: "user",
      content: "Remember the validation token"
    });
    expect(wrapper.find('[data-test="chat-permission-item-shortcuts"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain("Allow (Y)");
    expect(wrapper.text()).not.toContain("Deny once (D)");
  });
});

describe("ChatPermissionItem keyboard shortcuts (R5-pkb)", () => {
  function makeWrapper() {
    return mountItem({
      id: "perm_kbd",
      variant: "tool",
      toolId: "shell_exec",
      title: "Run command"
    });
  }

  it("makes the wrapping article keyboard-focusable", () => {
    const wrapper = makeWrapper();
    const root = wrapper.find('[data-test="chat-permission-item"]');
    expect(root.element.tagName).toBe("ARTICLE");
    expect(root.attributes("tabindex")).toBe("0");
  });

  it("renders shortcut hint labels for Allow / Deny / Deny once", () => {
    const wrapper = makeWrapper();
    const hints = wrapper.find('[data-test="chat-permission-item-shortcuts"]');
    expect(hints.exists()).toBe(true);
    const text = hints.text();
    expect(text).toContain("Allow (Y)");
    expect(text).toContain("Deny (N)");
    expect(text).toContain("Deny once (D)");
  });

  it("triggers Allow (resolve_permission grant) when Y is pressed", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "y" });
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_kbd",
      decision: "grant"
    });
  });

  it("triggers Allow when Enter is pressed", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "Enter" });
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_kbd",
      decision: "grant"
    });
  });

  it("triggers Deny (resolve_permission deny) when N is pressed", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "n" });
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_kbd",
      decision: "deny"
    });
  });

  it("triggers Deny when Escape is pressed", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "Escape" });
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_kbd",
      decision: "deny"
    });
  });

  it("falls back to Deny when D is pressed and no deny-once button exists", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "d" });
    expect(mockedInvoke).toHaveBeenCalledWith("resolve_permission", {
      requestId: "perm_kbd",
      decision: "deny"
    });
  });

  it("ignores unrelated keys", async () => {
    const wrapper = makeWrapper();
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "x" });
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "a" });
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("ignores Y / N / D when focus is in an editable element inside the item", async () => {
    const wrapper = makeWrapper();
    // The MCP trust checkbox is not rendered for non-MCP tool IDs, so
    // we inject a focusable input under the article to simulate any
    // future editable child without depending on MCP-specific UI.
    const root = wrapper.find('[data-test="chat-permission-item"]').element as HTMLElement;
    const input = document.createElement("input");
    input.type = "text";
    root.appendChild(input);
    input.focus();
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "y", bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "n", bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "d", bubbles: true }));
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("does not fire when keydown happens outside the item", async () => {
    makeWrapper();
    // Synthesize a keydown on the document body — the article's
    // @keydown only listens within its own subtree, so nothing should
    // dispatch resolve_permission.
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "y", bubbles: true }));
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("does not map D to reject_memory for memory prompts", async () => {
    const wrapper = mountItem({
      id: "mem_kbd",
      variant: "memory",
      toolId: "memory.store",
      title: "Save user memory"
    });
    await wrapper.find('[data-test="chat-permission-item"]').trigger("keydown", { key: "d" });
    expect(mockedInvoke).not.toHaveBeenCalled();
  });
});
