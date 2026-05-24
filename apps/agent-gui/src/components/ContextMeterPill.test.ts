import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ContextMeterPill from "@/components/ContextMeterPill.vue";
import ContextMeterDetails from "@/components/ContextMeterDetails.vue";
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

function mountPill() {
  return mountWithPlugins(ContextMeterPill, {
    reusePinia: true,
    mount: {
      global: {
        stubs: {
          Teleport: true
        }
      }
    }
  }).wrapper;
}

describe("ContextMeterPill (demoted compact surface)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    setActivePinia(createPinia());
    document.body.innerHTML = "";
  });

  it("renders compact tokens summary when usage data is present", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 12_400, budget_tokens: 180_000 });

    const wrapper = mountPill();
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="context-meter-pill-trigger"]');
    expect(trigger.exists()).toBe(true);

    const numbers = wrapper.find('[data-test="context-meter-pill-numbers"]');
    expect(numbers.exists()).toBe(true);
    // formatTokens compacts >= 1000 with "k" suffix and one decimal
    expect(numbers.text()).toContain("12.4k");
    expect(numbers.text()).toContain("180.0k");
  });

  it("shows the no-usage placeholder when usage is absent", async () => {
    const wrapper = mountPill();
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="context-meter-pill-trigger"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.text()).toContain("No usage yet");
  });

  it("opens the popover and renders ContextMeterDetails inside it", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 50, budget_tokens: 100 });

    const wrapper = mountPill();
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="context-meter-pill-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    const popover = wrapper.find('[data-test="context-meter-popover"]');
    expect(popover.exists()).toBe(true);
    expect(popover.classes()).toContain("kx-popover-content");
    expect(popover.classes()).toContain("context-meter-popover");
    expect(popover.find(".kx-popover-panel__header").exists()).toBe(true);

    const details = wrapper.findComponent(ContextMeterDetails);
    expect(details.exists()).toBe(true);
  });

  it("invokes compact_session when the inner compact action fires", async () => {
    invokeMock.mockResolvedValue(undefined);
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({
      total_tokens: 50,
      budget_tokens: 100,
      by_source: [["history", 50]]
    });

    const wrapper = mountPill();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-pill-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    const compactButton = wrapper.find('[data-test="context-meter-compact"]');
    expect(compactButton.exists()).toBe(true);
    expect(compactButton.attributes("disabled")).toBeUndefined();

    await compactButton.trigger("click");

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("compact_session");
  });

  it("does not re-invoke compact_session while a compaction is in flight", async () => {
    invokeMock.mockResolvedValue(undefined);
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({
      total_tokens: 50,
      budget_tokens: 100,
      by_source: [["history", 50]]
    });

    const wrapper = mountPill();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-pill-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="context-meter-compact"]').trigger("click");
    expect(invokeMock).toHaveBeenCalledTimes(1);

    session.compacting = true;
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-pill-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    const details = wrapper.findComponent(ContextMeterDetails);
    expect(details.exists()).toBe(true);
    details.vm.$emit("compact");
    await wrapper.vm.$nextTick();

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("applies warning and danger tones at the established thresholds", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 70, budget_tokens: 100 });

    const warnWrapper = mountPill();
    await warnWrapper.vm.$nextTick();
    expect(warnWrapper.find('[data-test="context-meter-pill-trigger"]').classes()).toContain(
      "pill-trigger--warn"
    );
    warnWrapper.unmount();

    session.lastContextUsage = makeUsage({ total_tokens: 85, budget_tokens: 100 });
    const dangerWrapper = mountPill();
    await dangerWrapper.vm.$nextTick();
    expect(dangerWrapper.find('[data-test="context-meter-pill-trigger"]').classes()).toContain(
      "pill-trigger--err"
    );
  });
});
