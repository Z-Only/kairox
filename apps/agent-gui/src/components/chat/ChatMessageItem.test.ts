import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatMessageItem from "@/components/chat/ChatMessageItem.vue";
import chatMessageItemSource from "@/components/chat/ChatMessageItem.vue?raw";

// ChatMessageItem itself does not call `useI18n` or `useRouter`, but we use
// `mountWithPlugins` to stay consistent with sibling chat-stream item specs
// and to make adding plugin-dependent assertions later painless.
function mountItem(props: Record<string, unknown>) {
  return mountWithPlugins(ChatMessageItem, {
    mount: { props },
    reusePinia: true
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("ChatMessageItem", () => {
  it("renders user role with raw content (no markdown)", () => {
    const wrapper = mountItem({ role: "user", content: "**bold**" });
    const root = wrapper.find('[data-test="chat-message"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-role")).toBe("user");
    const content = wrapper.find(".message-content");
    expect(content.exists()).toBe(true);
    // user branch does NOT render markdown
    expect(content.classes()).not.toContain("markdown-body");
    expect(content.text()).toBe("**bold**");
    expect(content.html()).not.toContain("<strong>");
  });

  it("renders assistant role through the markdown branch", () => {
    const wrapper = mountItem({ role: "assistant", content: "**bold**" });
    const root = wrapper.find('[data-test="chat-message"]');
    expect(root.attributes("data-role")).toBe("assistant");
    const body = wrapper.find(".markdown-body");
    expect(body.exists()).toBe(true);
    expect(body.html()).toContain("<strong>bold</strong>");
  });

  it("does not add extra vertical margins around single markdown paragraphs", () => {
    const wrapper = mountItem({ role: "assistant", content: "DONE" });

    expect(wrapper.find(".markdown-body p").exists()).toBe(true);
    expect(chatMessageItemSource).toContain(".markdown-body :deep(p) {\n  margin: 0;");
    expect(chatMessageItemSource).toContain(".markdown-body :deep(p + p)");
  });

  it("uses a block container for markdown lists so markers stay inside the message bubble", () => {
    const wrapper = mountItem({
      role: "assistant",
      content: "1. first item\n2. second item"
    });

    const body = wrapper.find(".message-content.markdown-body");
    expect(body.exists()).toBe(true);
    expect(body.element.tagName).toBe("DIV");
    expect(body.find("ol").exists()).toBe(true);
    expect(chatMessageItemSource).toContain("list-style-position: inside");
    expect(chatMessageItemSource).toContain("padding-left: 0");
  });

  it.each(["planner", "worker", "reviewer"] as const)(
    "renders %s role through the markdown branch",
    (role) => {
      const wrapper = mountItem({ role, content: "hello" });
      const root = wrapper.find('[data-test="chat-message"]');
      expect(root.attributes("data-role")).toBe(role);
      expect(wrapper.find(".markdown-body").exists()).toBe(true);
    }
  );

  it("marks [error]-prefixed content with data-error and error-banner test id", () => {
    const wrapper = mountItem({ role: "assistant", content: "[error] boom" });
    const root = wrapper.find('[data-test="chat-message"]');
    expect(root.attributes("data-error")).toBe("true");
    const banner = wrapper.find('[data-test="error-banner"]');
    expect(banner.exists()).toBe(true);
    expect(banner.classes()).toContain("markdown-body");
  });

  it("omits data-error when the content does not start with [error]", () => {
    const wrapper = mountItem({ role: "assistant", content: "all good" });
    const root = wrapper.find('[data-test="chat-message"]');
    expect(root.attributes("data-error")).toBeUndefined();
    expect(wrapper.find('[data-test="error-banner"]').exists()).toBe(false);
  });

  it('falls back to data-role="assistant" for an unknown role', () => {
    // Cast through `unknown` so we can pass an off-spec role value and verify
    // the runtime `|| 'assistant'` fallback that ChatPanel relies on.
    const wrapper = mountItem({ role: "mystery" as unknown as string, content: "hi" });
    const root = wrapper.find('[data-test="chat-message"]');
    expect(root.attributes("data-role")).toBe("assistant");
    expect(root.classes()).toContain("message-assistant");
  });
});
