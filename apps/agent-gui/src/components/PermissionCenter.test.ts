import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import PermissionCenter from "./PermissionCenter.vue";
import type { TraceEntryData } from "../types/trace";
import { mountWithPlugins } from "@/test-utils/mount";

// `PermissionCenter.vue` renders `<PermissionPrompt>`, which calls
// `useI18n()`; bare `mount()` therefore throws "Need to install with
// `app.use` function". `mountWithPlugins` installs i18n + router;
// `reusePinia: true` keeps the `beforeEach` pinia.
//
// Passing the extended-options shape returns `{ wrapper, router }`; we
// unwrap `.wrapper` so call-sites stay drop-in compatible with the prior
// `mount(...)` usage.
const mount = (comp: typeof PermissionCenter) =>
  mountWithPlugins(comp, { reusePinia: true }).wrapper;

// Use vi.hoisted so the mutable entries array is available inside vi.mock factories.
// We use a plain object (not reactive) because vi.hoisted runs before imports are resolved.
const { mockEntries } = vi.hoisted(() => ({
  mockEntries: [] as TraceEntryData[]
}));

// Mock Tauri APIs (required by PermissionPrompt child component)
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

// Mock the useTraceStore composable — PermissionCenter imports traceState directly
vi.mock("../composables/useTraceStore", () => ({
  get traceState() {
    return { entries: mockEntries };
  }
}));

function makeEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "permission",
    status: "pending",
    toolId: "shell_exec",
    title: "Run command",
    startedAt: Date.now(),
    expanded: true,
    ...overrides
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockEntries.length = 0;
});

describe("PermissionCenter", () => {
  it("renders the section heading Permissions", () => {
    const wrapper = mount(PermissionCenter);
    expect(wrapper.find("h2").text()).toBe("Permissions");
  });

  it("renders No pending requests when trace entries are empty", () => {
    const wrapper = mount(PermissionCenter);
    const empty = wrapper.get('[data-test="permission-empty-state"]');
    expect(empty.text()).toBe("No pending requests");
    expect(empty.classes()).toContain("kx-empty-state");
    expect(wrapper.classes()).not.toContain("permission-center--scrollable");
  });

  it("renders No pending requests when there are only completed permission entries", () => {
    mockEntries.push(
      makeEntry({ id: "perm_1", kind: "permission", status: "completed" }),
      makeEntry({ id: "perm_2", kind: "permission", status: "failed" })
    );
    const wrapper = mount(PermissionCenter);
    expect(wrapper.get('[data-test="permission-empty-state"]').text()).toBe("No pending requests");
  });

  it("renders PermissionPrompt for each pending permission entry", () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "pending",
        title: "Run ls"
      }),
      makeEntry({
        id: "perm_2",
        kind: "permission",
        status: "pending",
        title: "Run cat"
      })
    );
    const wrapper = mount(PermissionCenter);
    expect(wrapper.find('[data-test="permission-empty-state"]').exists()).toBe(false);
    expect(wrapper.classes()).toContain("permission-center--scrollable");
    const prompts = wrapper.findAllComponents({ name: "PermissionPrompt" });
    expect(prompts).toHaveLength(2);
  });

  it("renders PermissionPrompt for each pending memory entry", () => {
    mockEntries.push(
      makeEntry({
        id: "mem_1",
        kind: "memory",
        status: "pending",
        title: "Save memory",
        scope: "user",
        content: "pref"
      })
    );
    const wrapper = mount(PermissionCenter);
    expect(wrapper.find('[data-test="permission-empty-state"]').exists()).toBe(false);
    const prompts = wrapper.findAllComponents({ name: "PermissionPrompt" });
    expect(prompts).toHaveLength(1);
  });

  it("renders both permission and memory pending entries together", () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "pending",
        title: "Run ls"
      }),
      makeEntry({
        id: "mem_1",
        kind: "memory",
        status: "pending",
        title: "Save memory",
        scope: "user",
        content: "pref"
      })
    );
    const wrapper = mount(PermissionCenter);
    expect(wrapper.find('[data-test="permission-empty-state"]').exists()).toBe(false);
    const prompts = wrapper.findAllComponents({ name: "PermissionPrompt" });
    expect(prompts).toHaveLength(2);
  });

  it("renders pending request type filter chips with live counts", () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "pending",
        title: "Run ls"
      }),
      makeEntry({
        id: "perm_2",
        kind: "permission",
        status: "pending",
        title: "Run cat"
      }),
      makeEntry({
        id: "mem_1",
        kind: "memory",
        status: "pending",
        title: "Save memory",
        scope: "user",
        content: "pref"
      })
    );

    const wrapper = mount(PermissionCenter);

    expect(wrapper.find('[data-test="permission-type-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="permission-filter-all"]').text()).toBe("All 3");
    expect(wrapper.find('[data-test="permission-filter-tool"]').text()).toBe("Tools 2");
    expect(wrapper.find('[data-test="permission-filter-memory"]').text()).toBe("Memories 1");
  });

  it("filters pending requests to memory proposals", async () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "pending",
        title: "Run ls"
      }),
      makeEntry({
        id: "mem_1",
        kind: "memory",
        status: "pending",
        title: "Save memory",
        scope: "user",
        content: "pref"
      })
    );

    const wrapper = mount(PermissionCenter);

    await wrapper.find('[data-test="permission-filter-memory"]').trigger("click");

    expect(wrapper.find('[data-test="permission-filter-memory"]').attributes("aria-pressed")).toBe(
      "true"
    );
    expect(wrapper.text()).toContain("Save memory");
    expect(wrapper.text()).not.toContain("Run ls");
    expect(wrapper.findAllComponents({ name: "PermissionPrompt" })).toHaveLength(1);
  });

  it("shows a filter-specific empty state when no pending requests match", async () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "pending",
        title: "Run ls"
      })
    );

    const wrapper = mount(PermissionCenter);

    await wrapper.find('[data-test="permission-filter-memory"]').trigger("click");

    expect(wrapper.get('[data-test="permission-empty-state"]').text()).toBe(
      "No pending requests match this filter"
    );
  });

  it("does not render entries with non-pending status", () => {
    mockEntries.push(
      makeEntry({
        id: "perm_1",
        kind: "permission",
        status: "completed",
        title: "Done"
      }),
      makeEntry({
        id: "perm_2",
        kind: "permission",
        status: "failed",
        title: "Denied"
      }),
      makeEntry({
        id: "mem_1",
        kind: "memory",
        status: "completed",
        title: "Saved"
      }),
      makeEntry({
        id: "perm_3",
        kind: "permission",
        status: "pending",
        title: "Waiting"
      })
    );
    const wrapper = mount(PermissionCenter);
    // Only the pending one should render a PermissionPrompt
    const prompts = wrapper.findAllComponents({ name: "PermissionPrompt" });
    expect(prompts).toHaveLength(1);
  });
});
