import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import ContextMeter from "@/components/ContextMeter.vue";
import { useSessionStore } from "@/stores/session";
import type { ContextUsage } from "@/types";

// Mock the Tauri IPC layer so `invoke("compact_session")` is observable
// without requiring a real Tauri runtime.
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}));

// `useToast()` ultimately reaches `useUiStore()` which calls `useUpdater`
// internally; stub it out to keep the test surface to the component itself.
vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn()
  })
}));

function makeUsage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  // ContextSource serialises as snake_case (verified at
  // crates/agent-core/src/context_types.rs:5 — `#[serde(rename_all = "snake_case")]`).
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

describe("ContextMeter.vue", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    // We seed store state BEFORE mount, so we must own the active pinia
    // and ask `mountWithPlugins` to reuse it via `reusePinia: true`.
    setActivePinia(createPinia());
  });

  it("renders a placeholder when no usage is available yet", () => {
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    expect(wrapper.find('[data-test="context-meter-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);
  });

  it("renders compact empty ring trigger and fallback details when no usage is available yet", async () => {
    const session = useSessionStore();
    session.projection.messages = [{ role: "user", content: "hi" }] as never;
    const { wrapper } = mountWithPlugins(ContextMeter, {
      props: { variant: "ring" },
      mount: { props: { variant: "ring" } },
      reusePinia: true
    });

    const emptyRing = wrapper.find('[data-test="context-meter-ring-empty"]');
    const ringTrigger = wrapper.find('[data-test="context-meter-ring"]');

    expect(emptyRing.exists()).toBe(true);
    expect(emptyRing.attributes("aria-label")).toContain("No usage yet");
    expect(ringTrigger.exists()).toBe(true);
    expect(ringTrigger.attributes("aria-label")).toContain("No usage yet");
    expect(wrapper.find('[data-test="context-meter-empty"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);

    await ringTrigger.trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="context-meter-popover"]').text()).toContain("No usage yet");
  });

  it("renders the segmented bar and a healthy badge under 70%", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 90_000 }); // 50%
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-badge-warn"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="context-meter-badge-err"]').exists()).toBe(false);
  });

  it("renders compact ring variant with percent label", async () => {
    const session = useSessionStore();
    session.projection.messages = [{ role: "user", content: "hi" }] as never;
    session.lastContextUsage = makeUsage({
      total_tokens: 50,
      budget_tokens: 100,
      context_window: 120,
      output_reservation: 20,
      by_source: [["history", 50]],
      estimator: "cl100k_base",
      corrected_by_real_usage: false
    });
    const { wrapper } = mountWithPlugins(ContextMeter, {
      props: { variant: "ring" },
      mount: { props: { variant: "ring" } },
      reusePinia: true
    });
    await wrapper.vm.$nextTick();

    const ring = wrapper.find('[data-test="context-meter-ring"]');
    expect(ring.exists()).toBe(true);
    expect(ring.attributes("aria-label")).toContain("50");
    expect(ring.text()).toBe("50%");
  });

  it("shows the err badge above 85%", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage({ total_tokens: 160_000 }); // ~89%
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="context-meter-badge-err"]').exists()).toBe(true);
  });

  it("disables the Compact button while compacting", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage();
    session.compacting = true;
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    // Open the popover first.
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    const btn = wrapper.find<HTMLButtonElement>('[data-test="context-meter-compact"]');
    expect(btn.exists()).toBe(true);
    expect(btn.element.disabled).toBe(true);
  });

  it("invokes compact_session when Compact is clicked", async () => {
    invokeMock.mockResolvedValue(undefined);
    const session = useSessionStore();
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.find('[data-test="context-meter-compact"]').trigger("click");
    expect(invokeMock).toHaveBeenCalledWith("compact_session");
  });

  it("renders one popover row per source from by_source", async () => {
    const session = useSessionStore();
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    const rows = wrapper.findAll('[data-test^="context-meter-row-"]');
    expect(rows.length).toBe(4);
    expect(wrapper.find('[data-test="context-meter-reserved"]').exists()).toBe(true);
  });
});
