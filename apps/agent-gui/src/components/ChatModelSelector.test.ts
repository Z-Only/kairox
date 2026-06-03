import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import ChatModelSelector from "./ChatModelSelector.vue";
import { mountWithPlugins } from "@/test-utils/mount";
import type { ProfileInfo } from "@/types";

function profile(overrides: Partial<ProfileInfo> & Pick<ProfileInfo, "alias">): ProfileInfo {
  return {
    alias: overrides.alias,
    provider: "openai",
    model_id: overrides.alias,
    local: false,
    has_api_key: true,
    supports_reasoning: false,
    ...overrides
  };
}

const modelOptions = [
  profile({
    alias: "fast",
    model_id: "gpt-4o-mini",
    model_display: "GPT-4o Mini"
  }),
  profile({
    alias: "smart",
    model_id: "gpt-5.2",
    model_display: "GPT-5.2",
    supports_reasoning: true
  })
];

function mountSelector(props: Partial<InstanceType<typeof ChatModelSelector>["$props"]> = {}) {
  return mountWithPlugins(ChatModelSelector, {
    props: {
      modelOptions,
      currentProfile: "fast",
      switchingModel: false,
      activeProfileDisplay: "GPT-4o Mini",
      currentReasoningEffort: null,
      ...props
    }
  });
}

describe("ChatModelSelector", () => {
  it("renders the active model display on the trigger", () => {
    const wrapper = mountSelector({
      activeProfileDisplay: "OpenAI · GPT-5.2 · high",
      currentProfile: "smart",
      currentReasoningEffort: "high"
    });

    const trigger = wrapper.find('[data-test="chat-model-trigger"]');
    expect(trigger.text()).toBe("OpenAI · GPT-5.2 · high");
    expect(trigger.attributes("aria-label")).toBe(
      "Select model. Current model: OpenAI · GPT-5.2 · high"
    );
  });

  it("emits only the alias when selecting a non-reasoning model", async () => {
    const wrapper = mountSelector({ currentProfile: "smart" });

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-fast"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["fast"]]);
  });

  it("requests the popover to close after selecting a concrete model option", async () => {
    const wrapper = mountSelector({ currentProfile: "smart", open: true });

    await wrapper.find('[data-test="chat-model-option-fast"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["fast"]]);
    expect(wrapper.emitted("update:open")?.at(-1)).toEqual([false]);
  });

  it("hides the popover content after selecting a concrete model option", async () => {
    const wrapper = mountSelector({ currentProfile: "smart" });

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    expect(wrapper.find('[data-test="chat-model-popover"]').exists()).toBe(true);

    await wrapper.find('[data-test="chat-model-option-fast"]').trigger("click");
    await nextTick();

    expect(wrapper.find('[data-test="chat-model-popover"]').exists()).toBe(false);
  });

  it("emits the hovered reasoning model and built-in effort", async () => {
    const wrapper = mountSelector();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("focus");

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);

    await wrapper.find('[data-test="chat-reasoning-option-high"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["smart", "high"]]);
  });

  it("requests the popover to close after selecting a reasoning effort", async () => {
    const wrapper = mountSelector({ open: true });

    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("focus");
    await wrapper.find('[data-test="chat-reasoning-option-high"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["smart", "high"]]);
    expect(wrapper.emitted("update:open")?.at(-1)).toEqual([false]);
  });

  it("does not select a default reasoning effort when none is set", async () => {
    const wrapper = mountSelector({
      currentProfile: "smart",
      currentReasoningEffort: null
    });

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);
    expect(wrapper.findAll(".chat-reasoning-option.selected")).toHaveLength(0);
    expect(wrapper.findAll(".chat-reasoning-option.kx-popover-option--selected")).toHaveLength(0);
  });

  it("renders reasoning controls in a separate anchored card beside the model list", async () => {
    const wrapper = mountSelector();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("focus");

    const modelCard = wrapper.find('[data-test="chat-model-card"]');
    const reasoningPanel = wrapper.find('[data-test="chat-reasoning-panel"]');

    expect(modelCard.exists()).toBe(true);
    expect(reasoningPanel.exists()).toBe(true);
    expect(reasoningPanel.element.parentElement).not.toBe(modelCard.element);
    expect(reasoningPanel.classes()).toContain("chat-reasoning-panel--anchored");
    expect(reasoningPanel.attributes("style")).toContain("--chat-reasoning-anchor-y:");
  });

  it("keeps the reasoning card within the model card vertical bounds before centering", async () => {
    const wrapper = mountSelector();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");

    const modelCard = wrapper.find<HTMLElement>('[data-test="chat-model-card"]');
    const smartOption = wrapper.find<HTMLElement>('[data-test="chat-model-option-smart"]');
    Object.defineProperty(modelCard.element, "clientHeight", {
      configurable: true,
      value: 180
    });
    modelCard.element.getBoundingClientRect = () =>
      ({
        top: 100,
        bottom: 280,
        height: 180,
        left: 0,
        right: 360,
        width: 360,
        x: 0,
        y: 100,
        toJSON: () => ({})
      }) as DOMRect;
    smartOption.element.getBoundingClientRect = () =>
      ({
        top: 240,
        bottom: 280,
        height: 40,
        left: 0,
        right: 330,
        width: 330,
        x: 0,
        y: 240,
        toJSON: () => ({})
      }) as DOMRect;

    await smartOption.trigger("focus");
    await nextTick();

    const reasoningPanel = wrapper.find<HTMLElement>('[data-test="chat-reasoning-panel"]');
    reasoningPanel.element.getBoundingClientRect = () =>
      ({
        top: 0,
        bottom: 120,
        height: 120,
        left: 370,
        right: 586,
        width: 216,
        x: 370,
        y: 0,
        toJSON: () => ({})
      }) as DOMRect;

    await smartOption.trigger("focus");
    await nextTick();

    expect(reasoningPanel.attributes("style")).toContain("--chat-reasoning-anchor-y: 120px");
  });

  it("trims custom reasoning effort before emitting", async () => {
    const wrapper = mountSelector();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("mouseenter");
    await wrapper.find('[data-test="chat-reasoning-custom-input"]').setValue("  reasoning-max  ");
    await wrapper.find('[data-test="chat-reasoning-custom-apply"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["smart", "reasoning-max"]]);
  });
});
