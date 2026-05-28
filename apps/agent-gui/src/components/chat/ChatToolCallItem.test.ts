import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatToolCallItem from "@/components/chat/ChatToolCallItem.vue";
import { useSessionStore } from "@/stores/session";

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
  localStorage.clear();
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

  describe("timing breakdown", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("renders 'started 3s ago' below duration when expanded with startedAt", () => {
      const now = new Date("2026-05-25T12:00:00Z").getTime();
      vi.setSystemTime(now);

      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        durationMs: 1200,
        startedAt: now - 3000,
        defaultExpanded: true
      });

      expect(wrapper.find(".chat-tool-call__duration").text()).toBe("1.2s");
      const startedAgo = wrapper.find('[data-test="chat-tool-call-started-ago"]');
      expect(startedAgo.exists()).toBe(true);
      expect(startedAgo.text()).toBe("started 3s ago");
    });

    it("hides the 'started X ago' line when collapsed but keeps duration", () => {
      const now = new Date("2026-05-25T12:00:00Z").getTime();
      vi.setSystemTime(now);

      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        durationMs: 1200,
        startedAt: now - 3000
      });

      expect(wrapper.find(".chat-tool-call__duration").text()).toBe("1.2s");
      expect(wrapper.find('[data-test="chat-tool-call-started-ago"]').exists()).toBe(false);
    });

    it("renders 'failed after 5.0s' instead of plain duration when status is failed", () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "failed",
        durationMs: 5000
      });

      expect(wrapper.find(".chat-tool-call__duration").text()).toBe("failed after 5.0s");
    });

    it("renders 'started just now' for elapsed under 5 seconds", () => {
      const now = new Date("2026-05-25T12:00:00Z").getTime();
      vi.setSystemTime(now);

      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        durationMs: 500,
        startedAt: now - 2000,
        defaultExpanded: true
      });

      expect(wrapper.find('[data-test="chat-tool-call-started-ago"]').text()).toBe(
        "started just now"
      );
    });

    it("renders minute-grain relative time for older starts", () => {
      const now = new Date("2026-05-25T12:00:00Z").getTime();
      vi.setSystemTime(now);

      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        durationMs: 800,
        // 3m 20s = 200s
        startedAt: now - 200_000,
        defaultExpanded: true
      });

      expect(wrapper.find('[data-test="chat-tool-call-started-ago"]').text()).toBe(
        "started 3m 20s ago"
      );
    });
  });

  describe("keyboard accessibility (parity with ChatPermissionItem)", () => {
    it("makes the row keyboard-focusable with role=button and tabindex=0", () => {
      const wrapper = mountItem({ toolId: "shell_exec", status: "completed" });
      const row = wrapper.find(".chat-tool-call__row");
      expect(row.attributes("role")).toBe("button");
      expect(row.attributes("tabindex")).toBe("0");
    });

    it("reflects expand state via aria-expanded on the row", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      const row = wrapper.find(".chat-tool-call__row");
      expect(row.attributes("aria-expanded")).toBe("false");

      await row.trigger("click");

      expect(row.attributes("aria-expanded")).toBe("true");
    });

    it("links the expanded detail panel via aria-controls", () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls",
        defaultExpanded: true
      });
      const row = wrapper.find(".chat-tool-call__row");
      const detail = wrapper.find(".chat-tool-call__detail");
      const controls = row.attributes("aria-controls");
      expect(controls).toBeTruthy();
      expect(detail.attributes("id")).toBe(controls);
    });

    it("toggles expand state when Enter is pressed on the row", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);

      await wrapper.find(".chat-tool-call__row").trigger("keydown", { key: "Enter" });

      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
      const events = wrapper.emitted("update:expanded");
      expect(events).toBeTruthy();
      expect(events!.at(-1)).toEqual([true]);
    });

    it("toggles expand state when Space is pressed on the row", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);

      await wrapper.find(".chat-tool-call__row").trigger("keydown", { key: " " });

      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
    });

    it("calls preventDefault on Space to avoid page scroll", () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      const row = wrapper.find<HTMLElement>(".chat-tool-call__row");
      const event = new KeyboardEvent("keydown", { key: " ", cancelable: true });
      row.element.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(true);
    });

    it("ignores unrelated keys on the row", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      const row = wrapper.find(".chat-tool-call__row");
      await row.trigger("keydown", { key: "a" });
      await row.trigger("keydown", { key: "Tab" });
      await row.trigger("keydown", { key: "ArrowDown" });

      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);
      expect(wrapper.emitted("update:expanded")).toBeUndefined();
    });

    it("ignores keydown bubbling from the inner toggle button to avoid double-toggle", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls"
      });
      // Enter pressed while the inner KxIconButton is focused bubbles a
      // keydown event to the row. The row's handler must ignore it; the
      // button's native click activation will handle the toggle once.
      await wrapper
        .find('[data-test="chat-tool-call-toggle"]')
        .trigger("keydown", { key: "Enter" });

      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);
      expect(wrapper.emitted("update:expanded")).toBeUndefined();
    });

    it("emits update:expanded on Enter in controlled mode but does not flip local state", async () => {
      const wrapper = mountItem({
        toolId: "shell_exec",
        status: "completed",
        input: "ls",
        expanded: false
      });

      await wrapper.find(".chat-tool-call__row").trigger("keydown", { key: "Enter" });

      const events = wrapper.emitted("update:expanded");
      expect(events).toBeTruthy();
      expect(events!.at(-1)).toEqual([true]);
      // Local state did not flip because the prop drives rendering.
      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);
    });
  });

  it("persists uncontrolled expand state per session in localStorage", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_persist";

    const wrapper = mountItem({
      toolId: "shell_exec",
      toolCallId: "tc_persist_1",
      status: "completed",
      input: "ls"
    });

    // Initial render: collapsed (no stored value yet)
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(false);

    // Expand → writes `true` to the session-scoped key
    await wrapper.find(".chat-tool-call__row").trigger("click");
    expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(true);
    expect(localStorage.getItem("kairox.chatToolExpand.ses_persist.tc_persist_1")).toBe("true");

    // A freshly mounted instance with the same key restores the
    // expanded state — i.e., the user's choice survives reloads.
    const wrapper2 = mountItem({
      toolId: "shell_exec",
      toolCallId: "tc_persist_1",
      status: "completed",
      input: "ls"
    });
    expect(wrapper2.find(".chat-tool-call__detail").exists()).toBe(true);
  });
});
