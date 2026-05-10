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

  it("renders compact empty ring when no usage is available yet", () => {
    const { wrapper } = mountWithPlugins(ContextMeter, {
      props: { variant: "ring" },
      mount: { props: { variant: "ring" } },
      reusePinia: true
    });

    const emptyRing = wrapper.find('[data-test="context-meter-ring-empty"]');
    expect(emptyRing.exists()).toBe(true);
    expect(emptyRing.attributes("aria-label")).toContain("No usage yet");
    expect(wrapper.find('[data-test="context-meter-empty"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="context-meter-ring"]').exists()).toBe(false);
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

describe("ContextMeter.vue — Switch model dropdown (P4)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    setActivePinia(createPinia());
    // `openProfilePicker()` calls `list_profiles_with_limits` once and
    // caches the result; provide a two-profile fixture by default.
    invokeMock.mockImplementation(async (cmd: string, _args?: unknown) => {
      if (cmd === "list_profiles_with_limits") {
        return [
          {
            alias: "fast",
            provider: "openai",
            model_id: "gpt-4o-mini",
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry",
            has_api_key: true
          },
          {
            alias: "opus",
            provider: "anthropic",
            model_id: "claude-opus",
            context_window: 200_000,
            output_limit: 16_384,
            limit_source: "builtin_registry",
            has_api_key: true
          }
        ];
      }
      if (cmd === "switch_model") return null;
      return null;
    });
  });

  it("enables the switch-model button when a session is active and idle", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    const btn = wrapper.find('[data-test="context-meter-switch-model"]');
    expect(btn.exists()).toBe(true);
    expect(btn.attributes("disabled")).toBeUndefined();
  });

  it("keeps the switch-model button disabled while compacting", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.compacting = true;
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    const btn = wrapper.find('[data-test="context-meter-switch-model"]');
    expect(btn.attributes("disabled")).toBeDefined();
  });

  it("opens the profile picker when the switch-model button is clicked", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    // `openProfilePicker` awaits `invoke("list_profiles_with_limits")` — let the
    // microtask queue drain so the profile list renders.
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    const items = wrapper.findAll('[data-test^="context-meter-profile-"]');
    expect(items.length).toBe(2);
    expect(wrapper.find('[data-test="context-meter-profile-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-profile-opus"]').exists()).toBe(true);
    // The "(Current)" marker sits on the current alias.
    expect(wrapper.find('[data-test="context-meter-profile-fast"]').text()).toMatch(
      /current|当前/i
    );
  });

  it("calls switch_model with the selected alias and closes the popover", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-profile-opus"]').trigger("click");
    // Let the awaited `invoke("switch_model")` resolve.
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    expect(invokeMock).toHaveBeenCalledWith("switch_model", {
      sessionId: "ses_test",
      profileAlias: "opus"
    });
    // Popover should close after a successful switch.
    expect(wrapper.find('[data-test="context-meter-popover"]').exists()).toBe(false);
  });

  it("clicking the already-current profile is a no-op (no switch_model call)", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-profile-fast"]').trigger("click");
    await wrapper.vm.$nextTick();
    const switchCalls = invokeMock.mock.calls.filter((c) => c[0] === "switch_model");
    expect(switchCalls.length).toBe(0);
  });
});
