import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import MemoryBrowser from "./MemoryBrowser.vue";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useMemoryStore } from "@/stores/memory";
import { useSessionStore } from "@/stores/session";

// MemoryBrowser uses `useI18n()` and `useDialog()` (added in Task 7b
// when ConfirmDialog.vue was retired in favour of NaiveUI's dialog
// hook). `mountWithPlugins({ withNaiveProviders: true })` wires the
// same provider stack `AppLayout.vue` mounts at runtime so both hooks
// resolve cleanly inside vitest.
function mountBrowser() {
  return mountWithPlugins(MemoryBrowser, {
    withNaiveProviders: true
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

  it("changes active scope filter via NSelect", async () => {
    const { wrapper } = mountBrowser();
    await flushPromises();
    // Drive the production `NSelect` (the only scope UI) directly.
    // Look the component up via the stable `data-test` hook on its
    // root DOM node — NaiveUI registers the component as `Select`
    // (no `N` prefix) and the SFC renders multiple internal `Select`
    // subcomponents, so a `findComponent({ name: 'Select' })` query
    // is ambiguous. Going through the DOM hook gets us the outer
    // wrapper component whose `update:value` event the template
    // binds to `handleFilterChange`.
    const memory = useMemoryStore();
    const selectRoot = wrapper.find('[data-test="memory-scope-select"]');
    expect(selectRoot.exists()).toBe(true);
    const select = selectRoot.findComponent({ name: "Select" });
    expect(select.exists()).toBe(true);

    for (const scope of ["session", "user", "workspace", "all"] as const) {
      select.vm.$emit("update:value", scope);
      await flushPromises();
      expect(memory.filter).toBe(scope);
    }
  });
});
