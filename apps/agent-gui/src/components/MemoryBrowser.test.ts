import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import MemoryBrowser from "./MemoryBrowser.vue";
import memoryBrowserSource from "./MemoryBrowser.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useMemoryStore } from "@/stores/memory";
import { useSessionStore } from "@/stores/session";

// MemoryBrowser uses `useI18n()`. `mountWithPlugins` wires the i18n +
// Pinia + router plugin stack so hooks resolve cleanly inside vitest.
// We use the extended `MountWithPluginsOptions` shape so that the
// `global.provide` for `confirmDialogKey` is forwarded correctly inside
// `mount` and the return type is `{ wrapper, router }`.
function mountBrowser() {
  return mountWithPlugins(MemoryBrowser, {
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        }
      }
    }
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  // Default invoke mock: empty list (the component calls
  // `query_memories` on mount via `loadMemories`).
  mockedInvoke.mockResolvedValue([]);
});

describe("MemoryBrowser", () => {
  it("shows empty state when no memories", async () => {
    const { wrapper } = mountBrowser();
    const session = useSessionStore();
    session.currentSessionId = "ses_1";
    await flushPromises();
    expect(wrapper.text()).toContain("No memories");
  });

  it("shows loading state", async () => {
    const { wrapper } = mountBrowser();
    // Wait for the on-mount `loadMemories()` to settle FIRST — its
    // finally-block assigns `loading = false` and would otherwise
    // overwrite our test setup.
    await flushPromises();
    // `mountWithPlugins` activates a fresh Pinia internally, so the
    // store we read here is the same instance the component bound to.
    const memory = useMemoryStore();
    memory.loading = true;
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("Loading");
  });

  it("renders memory items with scope info", async () => {
    const { wrapper } = mountBrowser();
    // Wait for the on-mount `loadMemories()` (which `await`s
    // `query_memories` and overwrites `memories`) to settle BEFORE we
    // mutate the store, otherwise the async result `[]` clobbers our
    // direct assignment.
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Rust", accepted: true },
      {
        id: "m2",
        scope: "session",
        key: null,
        content: "Temp note",
        accepted: true
      }
    ];
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("Rust");
    expect(wrapper.text()).toContain("Temp note");
    expect(wrapper.text()).toContain("user");
  });

  it("renders memory status filter chips with live counts", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Rust", accepted: true },
      {
        id: "m2",
        scope: "workspace",
        key: "style",
        content: "Prefer concise UI copy",
        accepted: true
      },
      {
        id: "m3",
        scope: "session",
        key: null,
        content: "Draft preference",
        accepted: false
      }
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-status-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-status-filter-all"]').text()).toBe("All 3");
    expect(wrapper.find('[data-test="memory-status-filter-accepted"]').text()).toBe("Accepted 2");
    expect(wrapper.find('[data-test="memory-status-filter-pending"]').text()).toBe("Pending 1");
  });

  it("filters visible memories by pending status", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Accepted memory", accepted: true },
      {
        id: "m2",
        scope: "session",
        key: null,
        content: "Pending memory",
        accepted: false
      }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="memory-status-filter-pending"]').trigger("click");

    expect(
      wrapper.find('[data-test="memory-status-filter-pending"]').attributes("aria-pressed")
    ).toBe("true");
    expect(wrapper.text()).toContain("Pending memory");
    expect(wrapper.text()).not.toContain("Accepted memory");
  });

  it("sorts loaded memories after applying the status filter without reloading", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "workspace", key: "zeta", content: "Zoo memory", accepted: true },
      {
        id: "m2",
        scope: "user",
        key: "alpha",
        content: "Alpha accepted memory",
        accepted: true
      },
      {
        id: "m3",
        scope: "session",
        key: "beta",
        content: "Beta pending memory",
        accepted: false
      }
    ];
    await wrapper.vm.$nextTick();
    mockedInvoke.mockClear();

    await wrapper.find('[data-test="memory-status-filter-accepted"]').trigger("click");
    await wrapper.find('[data-test="memory-sort-select"]').setValue("key");

    expect(wrapper.findAll('[data-test="memory-item"]').map((item) => item.text())).toEqual([
      expect.stringContaining("Alpha accepted memory"),
      expect.stringContaining("Zoo memory")
    ]);
    expect(wrapper.text()).not.toContain("Beta pending memory");
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("shows a status-filter empty state when no memories match", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Accepted memory", accepted: true }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="memory-status-filter-pending"]').trigger("click");

    expect(wrapper.find('[data-test="memory-empty-state"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("No memories match this status filter");
  });

  it("shows accept and reject actions for pending memories", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Pending memory", accepted: false }
    ];
    const acceptSpy = vi.spyOn(memory, "acceptMemoryItem").mockResolvedValue();
    const rejectSpy = vi.spyOn(memory, "rejectMemoryItem").mockResolvedValue();
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="memory-accept-btn"]').trigger("click");
    await wrapper.find('[data-test="memory-reject-btn"]').trigger("click");

    expect(acceptSpy).toHaveBeenCalledWith("m1");
    expect(rejectSpy).toHaveBeenCalledWith("m1");
  });

  it("does not show accept or reject actions for accepted memories", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Accepted memory", accepted: true }
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-accept-btn"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="memory-reject-btn"]').exists()).toBe(false);
  });

  it("changes active scope filter via select element", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    const selectEl = wrapper.find('[data-test="memory-scope-select"]');
    expect(selectEl.exists()).toBe(true);

    // Spy on the store action — the SFC's `handleFilterChange` should
    // call `setMemoryFilter(scope)` for every scope the user selects.
    const setFilterSpy = vi.spyOn(memory, "setMemoryFilter");

    for (const scope of ["session", "user", "workspace", "all"] as const) {
      await selectEl.setValue(scope);
      expect(setFilterSpy).toHaveBeenCalledWith(scope);
      expect(memory.filter).toBe(scope);
    }
  });

  it("audit anchors: exposes stable populated memory pilot selectors", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: true
      }
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-browser"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-item"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-refresh-btn"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-delete-btn"]').exists()).toBe(true);
  });

  it("audit anchors: exposes stable pending memory approval selectors", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: false
      }
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-accept-btn"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="memory-reject-btn"]').exists()).toBe(true);
  });

  it("audit anchors: exposes stable empty memory pilot selector", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-empty-state"]').exists()).toBe(true);
  });

  it("audit accessibility: names the memory scope selector for assistive tech", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();

    expect(wrapper.find('[data-test="memory-scope-select"]').attributes("aria-label")).toBe(
      "Memory scope"
    );
  });

  it("audit accessibility: uses the high-contrast empty-state treatment", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    const memory = useMemoryStore();
    memory.memories = [];
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="memory-empty-state"]').classes()).toContain(
      "memory-empty-state"
    );
  });

  it("audit accessibility: uses high-contrast memory form controls", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();

    expect(wrapper.find('[data-test="memory-scope-select"]').classes()).toContain("kx-select");
    expect(wrapper.find('[data-test="memory-search-input"]').classes()).toContain("kx-input");
    expectSourceMigration(memoryBrowserSource, {
      required: ["KxInput", "KxSelect", "KxChipGroup", "KxChipButton"],
      forbidden: [".scope-select {", ".search-input {"]
    });
  });
});
