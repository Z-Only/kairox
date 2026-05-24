import { readFileSync } from "node:fs";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TraceEntryData } from "../types/trace";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);
const kxButtonSource = readFileSync("src/components/ui/KxButton.vue", "utf8");
const themeCss = readFileSync("src/styles/theme.css", "utf8");
const workbenchViewSource = readFileSync("src/views/WorkbenchView.vue", "utf8");
const traceTimelineSource = readFileSync("src/components/TraceTimeline.vue", "utf8");
const traceEntrySource = readFileSync("src/components/TraceEntry.vue", "utf8");
const taskStepsSource = readFileSync("src/components/TaskSteps.vue", "utf8");
const taskNodeSource = readFileSync("src/components/TaskNode.vue", "utf8");
const memoryBrowserSource = readFileSync("src/components/MemoryBrowser.vue", "utf8");

function getCustomProperties(css: string, selector: string) {
  const ruleStartIndex = css.indexOf(`${selector} {`);
  if (ruleStartIndex === -1) {
    throw new Error(`Missing CSS rule for ${selector}`);
  }

  const ruleBodyStartIndex = css.indexOf("{", ruleStartIndex) + 1;
  const ruleBodyEndIndex = css.indexOf("}", ruleBodyStartIndex);
  const ruleBody = css.slice(ruleBodyStartIndex, ruleBodyEndIndex);

  return Object.fromEntries(
    [...ruleBody.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map(([, propertyName, propertyValue]) => [
      propertyName,
      propertyValue.trim()
    ])
  );
}

function parseHexColor(hexColor: string) {
  const normalizedHex = hexColor.replace("#", "");
  return [0, 2, 4].map(
    (startIndex) => Number.parseInt(normalizedHex.slice(startIndex, startIndex + 2), 16) / 255
  );
}

function getRelativeLuminance(hexColor: string) {
  const [red, green, blue] = parseHexColor(hexColor).map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  );
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function getContrastRatio(foregroundColor: string, backgroundColor: string) {
  const foregroundLuminance = getRelativeLuminance(foregroundColor);
  const backgroundLuminance = getRelativeLuminance(backgroundColor);
  const lighterLuminance = Math.max(foregroundLuminance, backgroundLuminance);
  const darkerLuminance = Math.min(foregroundLuminance, backgroundLuminance);

  return (lighterLuminance + 0.05) / (darkerLuminance + 0.05);
}

// MemoryBrowser (rendered when the Memory tab is activated) calls
// `useI18n()` and `useConfirm()`, so any render path that mounts it
// requires the i18n plugin and the confirmDialog injection.
// `mountWithPlugins` wires i18n plus a fresh Pinia; we provide the
// confirm injection via `mount.global.provide`.
function mountTimeline() {
  return mountWithPlugins(TraceTimeline, {
    reusePinia: true,
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        }
      }
    }
  }).wrapper;
}

function makeTraceEntry(id: string, overrides?: Partial<TraceEntryData>): TraceEntryData {
  return {
    id,
    kind: "tool",
    status: "completed",
    title: `Trace ${id}`,
    startedAt: Date.now(),
    expanded: true,
    ...overrides
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  clearTrace();
  // MemoryBrowser calls `invoke('query_memories', ...)` on mount and
  // assigns the result to `memories.value`. Without a default resolved
  // value, vitest mocks return `undefined`, which makes `memories.length`
  // throw inside the template render. Supply a stable empty-array
  // default so any invoke call this test file does not override stays
  // well-typed.
  mockedInvoke.mockResolvedValue([]);
});

describe("TraceTimeline", () => {
  it("shows Trace tab as active by default", () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    expect(buttons[0].classes()).toEqual(expect.arrayContaining(["active", "kx-button--primary"]));
    expect(buttons[0].text()).toBe("Trace");
  });

  it("switches to Tasks tab when clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[1].trigger("click");
    expect(buttons[1].classes()).toContain("active");
  });

  it("switches to Memory tab when clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    await buttons[2].trigger("click");
    expect(buttons[2].classes()).toContain("active");
  });

  it("cycles density when density buttons are clicked", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    expect(traceState.density).toBe("L2");
    const densityButtons = wrapper.findAll(".density-toolbar .density-btn");
    expect(densityButtons[1].classes()).toContain("density-btn--active");
    expect(wrapper.find(".density-label").exists()).toBe(true);
    await densityButtons[2].trigger("click");
    expect(traceState.density).toBe("L3");
    await densityButtons[0].trigger("click");
    expect(traceState.density).toBe("L1");
  });

  it("renders trace status filter chips with live counts", () => {
    traceState.entries = [
      makeTraceEntry("running", { title: "Running trace", status: "running" }),
      makeTraceEntry("pending", { title: "Pending trace", status: "pending" }),
      makeTraceEntry("failed", { title: "Failed trace", status: "failed" }),
      makeTraceEntry("done", { title: "Done trace", status: "completed" })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    expect(wrapper.find('[data-test="trace-status-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="trace-filter-all"]').text()).toBe("All 4");
    expect(wrapper.find('[data-test="trace-filter-active"]').text()).toBe("Active 2");
    expect(wrapper.find('[data-test="trace-filter-failed"]').text()).toBe("Failed 1");
    expect(wrapper.find('[data-test="trace-filter-done"]').text()).toBe("Done 1");
  });

  it("renders a trace search input with shared input styling", () => {
    traceState.entries = [makeTraceEntry("build", { title: "Build trace", status: "completed" })];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    const search = wrapper.get('[data-test="trace-search-input"]');
    expect(search.classes()).toContain("kx-input");
    expect(search.attributes("type")).toBe("search");
    expect(search.attributes("aria-label")).toBe("Search trace events");
    expect(traceTimelineSource).toContain("trace-search-input");
  });

  it("filters visible trace entries by failed status", async () => {
    traceState.entries = [
      makeTraceEntry("pending", { title: "Pending trace", status: "pending" }),
      makeTraceEntry("failed", { title: "Failed trace", status: "failed" }),
      makeTraceEntry("done", { title: "Done trace", status: "completed" })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.find('[data-test="trace-filter-failed"]').trigger("click");

    expect(wrapper.find('[data-test="trace-filter-failed"]').attributes("aria-pressed")).toBe(
      "true"
    );
    expect(wrapper.text()).toContain("Failed trace");
    expect(wrapper.text()).not.toContain("Pending trace");
    expect(wrapper.text()).not.toContain("Done trace");
  });

  it("filters visible trace entries by title, tool id, reason, and input", async () => {
    traceState.entries = [
      makeTraceEntry("build", {
        title: "Build project",
        input: "cargo test --workspace",
        reason: "Verify release build"
      }),
      makeTraceEntry("read", {
        title: "Read project guide",
        toolId: "fs_read",
        input: "AGENTS.md",
        reason: "Inspect local instructions"
      })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const search = wrapper.get('[data-test="trace-search-input"]');

    await search.setValue("cargo test");

    expect(wrapper.text()).toContain("Build project");
    expect(wrapper.text()).not.toContain("Read project guide");

    await search.setValue("fs_read");

    expect(wrapper.text()).toContain("fs_read");
    expect(wrapper.text()).not.toContain("Build project");
  });

  it("filters visible memory trace entries by scope and content", async () => {
    traceState.entries = [
      makeTraceEntry("tool", { title: "Run ls", status: "completed" }),
      makeTraceEntry("memory", {
        kind: "memory",
        title: "Save release memory",
        scope: "workspace",
        content: "Prefer compact release summaries",
        status: "pending"
      })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.get('[data-test="trace-search-input"]').setValue("compact");

    expect(wrapper.text()).toContain("Save release memory");
    expect(wrapper.text()).toContain("Prefer compact release summaries");
    expect(wrapper.text()).not.toContain("Run ls");
  });

  it("combines trace search with the selected status filter", async () => {
    traceState.entries = [
      makeTraceEntry("failed", {
        title: "Network request failed",
        status: "failed"
      }),
      makeTraceEntry("done", {
        title: "Network request completed",
        status: "completed"
      })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.get('[data-test="trace-search-input"]').setValue("network");
    await wrapper.find('[data-test="trace-filter-failed"]').trigger("click");

    expect(wrapper.text()).toContain("Network request failed");
    expect(wrapper.text()).not.toContain("Network request completed");
  });

  it("filters visible trace entries by type while combining status and search filters", async () => {
    traceState.entries = [
      makeTraceEntry("tool-failed-build", {
        kind: "tool",
        title: "Build command failed",
        input: "cargo test --workspace",
        status: "failed"
      }),
      makeTraceEntry("tool-completed-build", {
        kind: "tool",
        title: "Build command completed",
        input: "cargo build --workspace",
        status: "completed"
      }),
      makeTraceEntry("permission-failed-build", {
        kind: "permission",
        title: "Approve build command",
        reason: "Needs cargo test permission",
        status: "failed"
      }),
      makeTraceEntry("memory-pending-build", {
        kind: "memory",
        title: "Remember build preference",
        scope: "workspace",
        content: "Prefer cargo test before release",
        status: "pending"
      })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    const typeSelect = wrapper.get('[data-test="trace-kind-select"]');
    expect(typeSelect.attributes("aria-label")).toBe("Trace type");

    await typeSelect.setValue("tool");
    expect(wrapper.text()).toContain("Build command failed");
    expect(wrapper.text()).toContain("Build command completed");
    expect(wrapper.text()).not.toContain("Approve build command");
    expect(wrapper.text()).not.toContain("Remember build preference");

    await typeSelect.setValue("permission");
    expect(wrapper.text()).toContain("Approve build command");
    expect(wrapper.text()).not.toContain("Build command failed");
    expect(wrapper.text()).not.toContain("Remember build preference");

    await typeSelect.setValue("memory");
    expect(wrapper.text()).toContain("Remember build preference");
    expect(wrapper.text()).not.toContain("Build command failed");
    expect(wrapper.text()).not.toContain("Approve build command");

    await typeSelect.setValue("tool");
    await wrapper.find('[data-test="trace-filter-failed"]').trigger("click");
    await wrapper.get('[data-test="trace-search-input"]').setValue("cargo test");

    expect(wrapper.text()).toContain("Build command failed");
    expect(wrapper.text()).not.toContain("Build command completed");
    expect(wrapper.text()).not.toContain("Approve build command");
    expect(wrapper.text()).not.toContain("Remember build preference");
  });

  it("shows a status-filter empty state when no trace entries match", async () => {
    traceState.entries = [makeTraceEntry("done", { title: "Done trace", status: "completed" })];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.find('[data-test="trace-filter-failed"]').trigger("click");

    expect(wrapper.text()).toContain("No matching trace events");
  });

  it("shows a filtered empty state when search has no matches", async () => {
    traceState.entries = [makeTraceEntry("done", { title: "Done trace", status: "completed" })];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.get('[data-test="trace-search-input"]').setValue("does-not-exist");

    expect(wrapper.text()).toContain("No matching trace events");
    expect(wrapper.findAll('[data-test="trace-entry"]')).toHaveLength(0);
  });

  it("audit anchors: exposes stable trace pilot selectors", async () => {
    traceState.entries = [
      {
        id: "trace-1",
        kind: "model",
        status: "completed",
        title: "Assistant response",
        startedAt: Date.now(),
        expanded: true
      }
    ];
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    expect(wrapper.find('[data-test="trace-timeline"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="trace-tab-memory"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="trace-entry"]').exists()).toBe(true);
  });

  it("audit anchors: exposes stable tasks tab pilot selector", async () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    const tasksTab = wrapper.find('[data-test="trace-tab-tasks"]');
    expect(tasksTab.exists()).toBe(true);
    await tasksTab.trigger("click");
    expect(wrapper.find('[data-test="task-steps"]').exists()).toBe(true);
  });

  it("audit contrast tokens: keeps active trace controls and density labels readable in dark theme", () => {
    const darkThemeProperties = getCustomProperties(themeCss, "html.dark");

    expectSourceMigration(kxButtonSource, {
      required: ["color: var(--app-primary-contrast-color, #fff);"]
    });
    expect(
      getContrastRatio(
        darkThemeProperties["--app-primary-contrast-color"],
        darkThemeProperties["--app-primary-color"]
      )
    ).toBeGreaterThanOrEqual(4.5);
    expect(
      getContrastRatio(
        darkThemeProperties["--app-text-color-3"],
        darkThemeProperties["--app-card-color"]
      )
    ).toBeGreaterThanOrEqual(4.5);
  });

  it("audit layout: keeps right sidebar trace and task lists inside their container", () => {
    expectSourceMigration(workbenchViewSource, {
      requiredPatterns: [/\.right-sidebar\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s]
    });
    expectSourceMigration(traceTimelineSource, {
      requiredPatterns: [
        /\.trace-timeline\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s,
        /\.trace-entries\s*{[^}]*box-sizing:\s*border-box;[^}]*max-width:\s*100%;[^}]*overflow-x:\s*hidden;/s
      ]
    });
    expectSourceMigration(traceEntrySource, {
      requiredPatterns: [
        /\.trace-entry\s*{[^}]*box-sizing:\s*border-box;[^}]*max-width:\s*100%;[^}]*overflow-x:\s*hidden;/s,
        /\.entry-row\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s
      ]
    });
    expectSourceMigration(taskStepsSource, {
      requiredPatterns: [
        /\.task-steps\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s,
        /\.task-tree-scroll\s*{[^}]*box-sizing:\s*border-box;[^}]*max-width:\s*100%;[^}]*overflow-x:\s*hidden;/s
      ]
    });
    expectSourceMigration(taskNodeSource, {
      requiredPatterns: [
        /\.task-node-wrapper\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s,
        /\.task-node\s*{[^}]*box-sizing:\s*border-box;[^}]*width:\s*100%;[^}]*max-width:\s*100%;/s,
        /\.task-row\s*{[^}]*min-width:\s*0;[^}]*max-width:\s*100%;/s
      ]
    });
  });

  it("audit layout: keeps right sidebar empty-state dashed boxes inside their scroll panes", () => {
    expectSourceMigration(traceTimelineSource, {
      requiredPatterns: [
        /\.trace-empty\s*{[^}]*box-sizing:\s*border-box;[^}]*width:\s*calc\(100% - 24px\);/s
      ]
    });
    expectSourceMigration(taskStepsSource, {
      requiredPatterns: [
        /\.task-empty\s*{[^}]*box-sizing:\s*border-box;[^}]*width:\s*calc\(100% - 24px\);/s
      ]
    });
    expectSourceMigration(memoryBrowserSource, {
      requiredPatterns: [
        /\.memory-panel-state\s*{[^}]*box-sizing:\s*border-box;[^}]*width:\s*calc\(100% - 24px\);/s
      ]
    });
  });
});
