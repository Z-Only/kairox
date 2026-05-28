import { describe, it, expect } from "vitest";
import ChatSandboxSelector from "./ChatSandboxSelector.vue";
import { mountWithPlugins } from "@/test-utils/mount";

const mount = (props: { sandboxPolicy: string }) =>
  mountWithPlugins(ChatSandboxSelector, { props });

describe("ChatSandboxSelector", () => {
  it("renders the trigger button", () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    expect(wrapper.find('[data-test="chat-sandbox-trigger"]').exists()).toBe(true);
  });

  it("displays translated label for known policy on trigger", () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.text().length).toBeGreaterThan(0);
  });

  it("displays different text for each known policy", () => {
    const readOnly = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    const workspaceWrite = mount({
      sandboxPolicy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    });
    const fullAccess = mount({ sandboxPolicy: '{"kind":"danger_full_access"}' });

    const readOnlyText = readOnly.find('[data-test="chat-sandbox-trigger"]').text();
    const wsWriteText = workspaceWrite.find('[data-test="chat-sandbox-trigger"]').text();
    const fullAccessText = fullAccess.find('[data-test="chat-sandbox-trigger"]').text();

    expect(new Set([readOnlyText, wsWriteText, fullAccessText]).size).toBe(3);
  });

  it("falls back to raw kind for unknown policy JSON", () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"custom_sandbox"}' });
    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.text()).toContain("custom_sandbox");
  });

  it("falls back to raw string for invalid JSON", () => {
    const wrapper = mount({ sandboxPolicy: "not-json" });
    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.text()).toContain("not-json");
  });

  it("renders option buttons after opening the popover", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");

    expect(wrapper.find('[data-test="chat-sandbox-option-read_only"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-sandbox-option-workspace_write"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-sandbox-option-danger_full_access"]').exists()).toBe(
      true
    );
  });

  it("marks the current policy option as selected", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"workspace_write"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");

    const selected = wrapper.find('[data-test="chat-sandbox-option-workspace_write"]');
    expect(selected.classes()).toContain("selected");
  });

  it("does not mark non-current options as selected", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");

    const other = wrapper.find('[data-test="chat-sandbox-option-danger_full_access"]');
    expect(other.classes()).not.toContain("selected");
  });

  it("emits selectSandbox with JSON string when an option is clicked", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-sandbox-option-danger_full_access"]').trigger("click");

    expect(wrapper.emitted("selectSandbox")).toBeTruthy();
    expect(wrapper.emitted("selectSandbox")![0]).toEqual(['{"kind":"danger_full_access"}']);
  });

  it("emits selectSandbox with workspace_write JSON", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-sandbox-option-workspace_write"]').trigger("click");

    expect(wrapper.emitted("selectSandbox")![0]).toEqual([
      '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    ]);
  });

  it("emits selectSandbox with read_only JSON", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"danger_full_access"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-sandbox-option-read_only"]').trigger("click");

    expect(wrapper.emitted("selectSandbox")![0]).toEqual(['{"kind":"read_only"}']);
  });

  it("sets aria-current on the active option", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"danger_full_access"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");

    const active = wrapper.find('[data-test="chat-sandbox-option-danger_full_access"]');
    expect(active.attributes("aria-current")).toBe("true");
  });

  it("does not set aria-current on inactive options", async () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"danger_full_access"}' });
    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");

    const inactive = wrapper.find('[data-test="chat-sandbox-option-read_only"]');
    expect(inactive.attributes("aria-current")).toBeUndefined();
  });

  it("sets aria-label on trigger with current sandbox display", () => {
    const wrapper = mount({ sandboxPolicy: '{"kind":"read_only"}' });
    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.attributes("aria-label")).toContain(trigger.text());
  });
});
