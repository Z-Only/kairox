import { describe, expect, it } from "vitest";
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

  it("emits the hovered reasoning model and built-in effort", async () => {
    const wrapper = mountSelector();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("focus");

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);

    await wrapper.find('[data-test="chat-reasoning-option-high"]').trigger("click");

    expect(wrapper.emitted("selectModel")).toEqual([["smart", "high"]]);
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
