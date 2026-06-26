import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatCompactionItem from "@/components/chat/ChatCompactionItem.vue";

const ROOT = '[data-test="chat-compaction-item"]';

describe("ChatCompactionItem", () => {
  it("renders nothing when status is Idle", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: { status: { type: "Idle" } }
    });

    expect(wrapper.find(ROOT).exists()).toBe(false);
  });

  it("shows an aria-busy progress bar and user-requested reason chip while running", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: { type: "Running" },
        reason: { type: "UserRequested" }
      }
    });

    const root = wrapper.find(ROOT);
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("running");

    const bar = wrapper.find('[data-test="chat-compaction-bar"]');
    expect(bar.exists()).toBe(true);
    expect(bar.attributes("aria-busy")).toBe("true");

    const reason = wrapper.find('[data-test="chat-compaction-reason"]');
    expect(reason.exists()).toBe(true);
    expect(reason.text()).toBe("User-requested");
  });

  it("renders the threshold reason chip when the trigger was automatic", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: { type: "Running" },
        reason: { type: "Threshold", ratio: 0.85 }
      }
    });

    const reason = wrapper.find('[data-test="chat-compaction-reason"]');
    expect(reason.exists()).toBe(true);
    expect(reason.text()).toBe("Auto (threshold)");
  });

  it("formats ratio as a percentage and duration in seconds when completed", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: { type: "Completed" },
        ratio: 0.42,
        durationMs: 1234
      }
    });

    const root = wrapper.find(ROOT);
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("completed");
    expect(root.text()).toContain("42%");
    expect(root.text()).toContain("1.2s");
  });

  it("shows the failure message and the fallback chip when fallback was used", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: { type: "Failed", error: "boom" },
        fallbackUsed: true
      }
    });

    const root = wrapper.find(ROOT);
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("failed");

    const error = wrapper.find('[data-test="chat-compaction-error"]');
    expect(error.exists()).toBe(true);
    expect(error.text()).toBe("boom");

    const fallback = wrapper.find('[data-test="chat-compaction-fallback"]');
    expect(fallback.exists()).toBe(true);
    expect(fallback.text()).toBe("Sliding-window fallback");
  });

  it("renders the skipped status with the alreadyCompacting reason chip and ratio", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: {
          type: "Skipped",
          reason: { type: "AlreadyCompacting" },
          ratio: 0.5
        }
      }
    });

    const root = wrapper.find('[data-test="chat-compaction-skipped"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("skipped");
    expect(root.text()).toContain("Context compaction skipped");

    const reason = wrapper.find('[data-test="chat-compaction-skipped-reason"]');
    expect(reason.exists()).toBe(true);
    expect(reason.text()).toBe("another compaction in flight");

    const ratio = wrapper.find('[data-test="chat-compaction-ratio"]');
    expect(ratio.exists()).toBe(true);
    expect(ratio.text()).toContain("50%");
  });

  it("renders the thresholdDisabled reason chip and hides ratio when ratio is null", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: {
          type: "Skipped",
          reason: { type: "ThresholdDisabled" },
          ratio: null
        }
      }
    });

    const root = wrapper.find('[data-test="chat-compaction-skipped"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("skipped");

    const reason = wrapper.find('[data-test="chat-compaction-skipped-reason"]');
    expect(reason.exists()).toBe(true);
    expect(reason.text()).toBe("threshold disabled");

    expect(wrapper.find('[data-test="chat-compaction-ratio"]').exists()).toBe(false);
  });

  it("renders the notEnoughHistory reason chip and hides a non-informative ratio", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: {
          type: "Skipped",
          reason: { type: "NotEnoughHistory" },
          ratio: 0
        }
      }
    });

    const root = wrapper.find('[data-test="chat-compaction-skipped"]');
    expect(root.exists()).toBe(true);
    expect(root.attributes("data-status")).toBe("skipped");

    const reason = wrapper.find('[data-test="chat-compaction-skipped-reason"]');
    expect(reason.exists()).toBe(true);
    expect(reason.text()).toBe("not enough history");

    expect(wrapper.find('[data-test="chat-compaction-ratio"]').exists()).toBe(false);
    expect(root.text()).not.toContain("0%");
  });

  it("omits the fallback chip when fallback was not used", () => {
    const wrapper = mountWithPlugins(ChatCompactionItem, {
      props: {
        status: { type: "Failed", error: "boom" },
        fallbackUsed: false
      }
    });

    expect(wrapper.find('[data-test="chat-compaction-error"]').text()).toBe("boom");
    expect(wrapper.find('[data-test="chat-compaction-fallback"]').exists()).toBe(false);
  });
});
