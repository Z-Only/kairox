import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ContextMeter from "@/components/ContextMeter.vue";
import { useSessionStore } from "@/stores/session";
import type { ContextUsage } from "@/types";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}));

vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn()
  })
}));

function makeUsage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  return {
    total_tokens: 90_000,
    budget_tokens: 180_000,
    context_window: 200_000,
    output_reservation: 20_000,
    by_source: [
      ["system", 2_000],
      ["tool_definitions", 22_000],
      ["history", 60_000],
      ["memory", 6_000]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false,
    ...overrides
  };
}

function mountRingMeter() {
  return mountWithPlugins(ContextMeter, {
    reusePinia: true,
    mount: {
      props: { variant: "ring" },
      global: {
        stubs: {
          Teleport: true
        }
      }
    }
  }).wrapper;
}

describe("ContextMeter ring mode", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    setActivePinia(createPinia());
  });

  it("renders KxProgressRing inside the interactive ring trigger", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 50, budget_tokens: 100 });

    const wrapper = mountRingMeter();
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="context-meter-ring"]');
    const progressRing = wrapper.find('[data-test="context-progress-ring"]');

    expect(trigger.exists()).toBe(true);
    expect(progressRing.exists()).toBe(true);
    expect(progressRing.attributes("role")).toBe("progressbar");
    expect(progressRing.text()).toContain("50%");
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);
  });

  it("opens details in a KxPopover with stable content selector", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 50, budget_tokens: 100 });

    const wrapper = mountRingMeter();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-ring"]').trigger("click");
    await wrapper.vm.$nextTick();

    const popover = wrapper.find('[data-test="context-meter-popover"]');
    expect(popover.exists()).toBe(true);
    expect(popover.text()).toContain("Used tokens");
    expect(popover.text()).toContain("Max tokens");
    expect(popover.text()).toContain("Percentage");
    expect(popover.text()).toContain("Context window");
  });

  it("distinguishes warning and danger ring states at existing thresholds", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 70, budget_tokens: 100 });

    const warningWrapper = mountRingMeter();
    await warningWrapper.vm.$nextTick();
    expect(warningWrapper.find('[data-test="context-progress-ring"]').classes()).toContain(
      "kx-progress-ring--warning"
    );
    warningWrapper.unmount();

    session.lastContextUsage = makeUsage({ total_tokens: 85, budget_tokens: 100 });
    const dangerWrapper = mountRingMeter();
    await dangerWrapper.vm.$nextTick();
    expect(dangerWrapper.find('[data-test="context-progress-ring"]').classes()).toContain(
      "kx-progress-ring--danger"
    );
  });

  it("shows fallback details without rendering the old bar when usage is unavailable", async () => {
    const wrapper = mountRingMeter();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-ring"]').trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="context-meter-ring-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-popover"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-popover"]').text()).toContain("No usage yet");
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);
  });

  it("keeps source percentages finite when the token budget is zero", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({
      total_tokens: 50,
      budget_tokens: 0,
      by_source: [["history", 50]]
    });

    const ringWrapper = mountRingMeter();
    await ringWrapper.vm.$nextTick();
    await ringWrapper.find('[data-test="context-meter-ring"]').trigger("click");
    await ringWrapper.vm.$nextTick();

    const ringPopoverText = ringWrapper.find('[data-test="context-meter-popover"]').text();
    expect(ringPopoverText).not.toContain("Infinity");
    expect(ringPopoverText).not.toContain("NaN");
    expect(ringWrapper.find('[data-test="context-meter-row-history"]').text()).toContain("0");
    ringWrapper.unmount();

    const { wrapper: barWrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await barWrapper.vm.$nextTick();
    await barWrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await barWrapper.vm.$nextTick();

    const barPopoverText = barWrapper.find('[data-test="context-meter-popover"]').text();
    expect(barPopoverText).not.toContain("Infinity");
    expect(barPopoverText).not.toContain("NaN");
    expect(barWrapper.find('[data-test="context-meter-row-history"]').text()).toContain("0");
  });

  it("shows a readable fallback for unknown context sources", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({
      total_tokens: 12,
      budget_tokens: 100,
      by_source: [["future_source", 12] as never]
    });

    const wrapper = mountRingMeter();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-ring"]').trigger("click");
    await wrapper.vm.$nextTick();

    const unknownRow = wrapper.find('[data-test="context-meter-row-future_source"]');
    expect(unknownRow.exists()).toBe(true);
    expect(unknownRow.text()).toContain("future_source");
  });
});
