import { describe, it, expect } from "vitest";
import ChatApprovalSelector from "./ChatApprovalSelector.vue";
import { mountWithPlugins } from "@/test-utils/mount";

const mount = (props: { approvalPolicy: string }) =>
  mountWithPlugins(ChatApprovalSelector, { props });

describe("ChatApprovalSelector", () => {
  it("renders the trigger button", () => {
    const wrapper = mount({ approvalPolicy: "never" });
    expect(wrapper.find('[data-test="chat-approval-trigger"]').exists()).toBe(true);
  });

  it("displays translated label for known policy on trigger", () => {
    const wrapper = mount({ approvalPolicy: "never" });
    const trigger = wrapper.find('[data-test="chat-approval-trigger"]');
    // The i18n key resolves in the test locale; just confirm it renders non-empty text
    expect(trigger.text().length).toBeGreaterThan(0);
  });

  it("displays different text for each known policy", () => {
    const never = mount({ approvalPolicy: "never" });
    const onRequest = mount({ approvalPolicy: "on_request" });
    const always = mount({ approvalPolicy: "always" });

    const neverText = never.find('[data-test="chat-approval-trigger"]').text();
    const onRequestText = onRequest.find('[data-test="chat-approval-trigger"]').text();
    const alwaysText = always.find('[data-test="chat-approval-trigger"]').text();

    // All three should be different labels
    expect(new Set([neverText, onRequestText, alwaysText]).size).toBe(3);
  });

  it("falls back to raw value for unknown policy", () => {
    const wrapper = mount({ approvalPolicy: "custom_unknown" });
    const trigger = wrapper.find('[data-test="chat-approval-trigger"]');
    expect(trigger.text()).toContain("custom_unknown");
  });

  it("renders option buttons after opening the popover", async () => {
    const wrapper = mount({ approvalPolicy: "never" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");

    expect(wrapper.find('[data-test="chat-approval-option-never"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-approval-option-on_request"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-approval-option-always"]').exists()).toBe(true);
  });

  it("marks the current policy option as selected", async () => {
    const wrapper = mount({ approvalPolicy: "on_request" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");

    const selected = wrapper.find('[data-test="chat-approval-option-on_request"]');
    expect(selected.classes()).toContain("selected");
  });

  it("does not mark non-current options as selected", async () => {
    const wrapper = mount({ approvalPolicy: "never" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");

    const other = wrapper.find('[data-test="chat-approval-option-always"]');
    expect(other.classes()).not.toContain("selected");
  });

  it("emits selectApproval when an option is clicked", async () => {
    const wrapper = mount({ approvalPolicy: "never" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-approval-option-always"]').trigger("click");

    expect(wrapper.emitted("selectApproval")).toBeTruthy();
    expect(wrapper.emitted("selectApproval")![0]).toEqual(["always"]);
  });

  it("emits selectApproval with on_request value", async () => {
    const wrapper = mount({ approvalPolicy: "never" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-approval-option-on_request"]').trigger("click");

    expect(wrapper.emitted("selectApproval")![0]).toEqual(["on_request"]);
  });

  it("sets aria-current on the active option", async () => {
    const wrapper = mount({ approvalPolicy: "always" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");

    const active = wrapper.find('[data-test="chat-approval-option-always"]');
    expect(active.attributes("aria-current")).toBe("true");
  });

  it("does not set aria-current on inactive options", async () => {
    const wrapper = mount({ approvalPolicy: "always" });
    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");

    const inactive = wrapper.find('[data-test="chat-approval-option-never"]');
    expect(inactive.attributes("aria-current")).toBeUndefined();
  });

  it("sets aria-label on trigger with current approval display", () => {
    const wrapper = mount({ approvalPolicy: "never" });
    const trigger = wrapper.find('[data-test="chat-approval-trigger"]');
    expect(trigger.attributes("aria-label")).toContain(trigger.text());
  });
});
