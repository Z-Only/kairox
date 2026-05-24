import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatToolCallItem from "@/components/chat/ChatToolCallItem.vue";

// ChatToolCallItem calls `useI18n()` and uses auto-registered UI-kit
// components (KxTag, KxBadge, KxIconButton), so we mount via
// `mountWithPlugins` to install i18n + router. We pass the extended
// options shape and unwrap `.wrapper` for ergonomic call sites.
function mountItem(props: Record<string, unknown>) {
  return mountWithPlugins(ChatToolCallItem, {
    mount: { props },
    reusePinia: true
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("ChatToolCallItem", () => {
  it("renders with required props and shows tool id + status icon", () => {
    const wrapper = mountItem({ toolId: "shell_exec", status: "running" });

    expect(wrapper.find('[data-test="chat-tool-call-item"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("shell_exec");
    // running status uses the ⏳ glyph
    expect(wrapper.find(".chat-tool-call__status").text()).toBe("⏳");
  });

  it("renders status icons for each lifecycle state", () => {
    const cases: Array<[string, string]> = [
      ["running", "⏳"],
      ["completed", "✅"],
      ["failed", "❌"],
      ["pending", "🔑"]
    ];
    for (const [status, glyph] of cases) {
      const wrapper = mountItem({ toolId: "tool", status });
      expect(wrapper.find(".chat-tool-call__status").text()).toBe(glyph);
    }
  });

  it("hides input/output by default and reveals them after a row click", async () => {
    const wrapper = mountItem({
      toolId: "shell_exec",
      status: "completed",
      input: "ls -la",
      outputPreview: "total 0"
    });

    // Collapsed: no detail panel
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("ls -la");
    expect(wrapper.text()).not.toContain("total 0");

    await wrapper.find(".chat-tool-call__row").trigger("click");

    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
    expect(wrapper.text()).toContain("ls -la");
    expect(wrapper.text()).toContain("total 0");
  });

  it("renders input/output on initial mount when defaultExpanded is true", () => {
    const wrapper = mountItem({
      toolId: "shell_exec",
      status: "completed",
      input: "echo hi",
      outputPreview: "hi",
      defaultExpanded: true
    });

    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
    expect(wrapper.text()).toContain("echo hi");
    expect(wrapper.text()).toContain("hi");
  });

  it("emits update:expanded but stays driven by the prop in controlled mode", async () => {
    const wrapper = mountItem({
      toolId: "shell_exec",
      status: "completed",
      input: "ls",
      // Controlled: starts collapsed
      expanded: false
    });

    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);

    await wrapper.find(".chat-tool-call__row").trigger("click");

    // Emitted negated value
    const events = wrapper.emitted("update:expanded");
    expect(events).toBeTruthy();
    expect(events!.at(-1)).toEqual([true]);

    // Local state did NOT flip because the prop drives rendering
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);

    // When the parent flips the prop, the rendered state follows
    await wrapper.setProps({ expanded: true });
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);

    // And clicking again emits the next negation
    await wrapper.find(".chat-tool-call__row").trigger("click");
    const events2 = wrapper.emitted("update:expanded");
    expect(events2!.at(-1)).toEqual([false]);
  });

  it("applies the failed modifier class for failed status", () => {
    const wrapper = mountItem({ toolId: "shell_exec", status: "failed" });
    expect(wrapper.find('[data-test="chat-tool-call-item"]').classes()).toContain(
      "chat-tool-call--failed"
    );
  });

  it("renders duration as `1.2s` when durationMs=1234", () => {
    const wrapper = mountItem({
      toolId: "shell_exec",
      status: "completed",
      durationMs: 1234
    });
    expect(wrapper.find(".chat-tool-call__duration").text()).toBe("1.2s");
  });

  it("renders the scope chip when scope is provided", () => {
    const wrapper = mountItem({
      toolId: "memory.store",
      status: "completed",
      scope: "user"
    });
    const scope = wrapper.find(".chat-tool-call__scope");
    expect(scope.exists()).toBe(true);
    expect(scope.text()).toContain("user");
  });

  it("keeps the full toolId in the title attribute when long", () => {
    const longId = "very_long_tool_identifier_that_should_be_truncated_visually_xxxxxxxxxxxxxxxx";
    const wrapper = mountItem({ toolId: longId, status: "running" });
    const toolText = wrapper.find(".chat-tool-call__tool-text");
    expect(toolText.attributes("title")).toBe(longId);
    expect(toolText.classes()).toContain("chat-tool-call__tool-text");
  });

  it("toggle button also flips state in uncontrolled mode", async () => {
    const wrapper = mountItem({
      toolId: "shell_exec",
      status: "completed",
      input: "ls"
    });
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);
    await wrapper.find('[data-test="chat-tool-call-toggle"]').trigger("click");
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
  });
});
