import { readFileSync } from "node:fs";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import { useSessionStore } from "@/stores/session";
import { useUiStore } from "@/stores/ui";
import type { TraceEntryData } from "../types/trace";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

const mockGeneratedCommands = vi.hoisted(() => ({
  exportSessionDiagnostics: vi.fn(),
  listTrajectories: vi.fn(),
  getTrajectorySteps: vi.fn(),
  exportTrajectory: vi.fn()
}));

vi.mock("@/generated/commands", () => ({
  commands: mockGeneratedCommands
}));
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
function mountTimeline(locale: "en" | "zh-CN" = "en") {
  return mountWithPlugins(TraceTimeline, {
    reusePinia: true,
    locale,
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: {
            confirm: vi.fn().mockResolvedValue(true)
          }
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
  mockGeneratedCommands.exportSessionDiagnostics.mockReset();
  mockGeneratedCommands.listTrajectories.mockReset();
  mockGeneratedCommands.getTrajectorySteps.mockReset();
  mockGeneratedCommands.exportTrajectory.mockReset();
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

  it("switches to Subagents tab and renders the subagent panel", async () => {
    const wrapper = mountTimeline();
    await wrapper.get('[data-test="trace-tab-subagents"]').trigger("click");
    expect(useWorkspaceUiStore().rightPanelTab).toBe("subagents");
    expect(wrapper.get('[data-test="subagent-panel"]').exists()).toBe(true);
  });

  it("hides Changes tab for ordinary sessions", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_plain";
    session.sessions = [
      {
        id: "ses_plain",
        title: "Plain chat",
        profile: "fast",
        project_id: null,
        worktree_path: null,
        branch: null,
        visibility: null,
        deleted_at: null,
        approval_policy: null,
        sandbox_policy: null
      }
    ];
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.rightPanelTab = "changes";

    const wrapper = mountTimeline();
    await flushPromises();

    expect(wrapper.find('[data-test="trace-tab-changes"]').exists()).toBe(false);
    expect(workspaceUi.rightPanelTab).toBe("trace");
    expect(wrapper.find('[data-test="git-review-panel"]').exists()).toBe(false);
    expect(mockedInvoke).not.toHaveBeenCalledWith("get_session_git_review", {
      sessionId: "ses_plain"
    });
  });

  it("keeps right sidebar tabs within the sidebar when they need multiple rows", () => {
    expect(traceTimelineSource).toMatch(/\.trace-header\s*\{[^}]*align-items:\s*flex-start;/s);
    expect(traceTimelineSource).toMatch(/\.tab-group\s*\{[^}]*flex-wrap:\s*wrap;/s);
    expect(traceTimelineSource).toMatch(/\.tab-group\s*\{[^}]*max-width:\s*100%;/s);
  });

  it("switches to Changes tab and renders repository review", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_session_git_review") {
        return {
          kind: "dirty",
          branch: "feat/review",
          worktree_path: "/repo",
          message: null,
          file_count: 1,
          additions: 1,
          deletions: 0,
          changed_files: ["README.md"],
          staged: null,
          unstaged: {
            label: "Unstaged changes",
            stat: " README.md | 1 +",
            diff: "--- a/README.md\n+++ b/README.md\n+local agent edit",
            additions: 1,
            deletions: 0,
            files: [
              {
                path: "README.md",
                additions: 1,
                deletions: 0,
                diff: "--- a/README.md\n+++ b/README.md\n+local agent edit"
              }
            ]
          },
          untracked: null
        };
      }
      return [];
    });
    const session = useSessionStore();
    session.currentSessionId = "ses_1";
    session.sessions = [
      {
        id: "ses_1",
        title: "Review session",
        profile: "fast",
        project_id: "project_1",
        worktree_path: "/repo",
        branch: "feat/review",
        visibility: null,
        deleted_at: null,
        approval_policy: null,
        sandbox_policy: null
      }
    ];
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    await wrapper.get('[data-test="trace-tab-changes"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_review", {
      sessionId: "ses_1"
    });
    const workspaceUi = useWorkspaceUiStore();
    expect(workspaceUi.rightPanelTab).toBe("changes");
    expect(wrapper.get('[data-test="trace-tab-changes"]').classes()).toContain("active");
    expect(wrapper.get('[data-test="git-review-panel"]').text()).toContain("README.md");
    expect(wrapper.get('[data-test="git-review-panel"]').text()).toContain("local agent edit");
  });

  it("copies active session diagnostics to clipboard", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_trace";
    const ui = useUiStore();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    mockGeneratedCommands.exportSessionDiagnostics.mockResolvedValue({
      status: "ok",
      data: {
        session_id: "ses_trace",
        event_count: 2,
        event_type_counts: [{ event_type: "UserMessage", count: 1 }],
        last_event_type: "AssistantMessageCompleted",
        user_messages: [],
        assistant_messages: [],
        model_tool_calls: [],
        mcp_tool_calls: [],
        trajectory_started_count: 0,
        trajectory_completed_count: 0,
        trajectory_completed_outcomes: [],
        running_model_requests: 0,
        running_tool_invocations: 0,
        trajectory_failed_count: 0,
        has_terminal_assistant_message: true
      }
    });

    const wrapper = mountTimeline();

    await wrapper.get('[data-test="trace-copy-diagnostics"]').trigger("click");
    await wrapper.vm.$nextTick();

    expect(mockGeneratedCommands.exportSessionDiagnostics).toHaveBeenCalledWith("ses_trace");
    expect(writeText).toHaveBeenCalledWith(
      JSON.stringify({
        session_id: "ses_trace",
        event_count: 2,
        event_type_counts: [{ event_type: "UserMessage", count: 1 }],
        last_event_type: "AssistantMessageCompleted",
        user_messages: [],
        assistant_messages: [],
        model_tool_calls: [],
        mcp_tool_calls: [],
        trajectory_started_count: 0,
        trajectory_completed_count: 0,
        trajectory_completed_outcomes: [],
        running_model_requests: 0,
        running_tool_invocations: 0,
        trajectory_failed_count: 0,
        has_terminal_assistant_message: true
      })
    );
    expect(ui.toasts.at(-1)).toMatchObject({
      message: "Session diagnostics copied",
      type: "success"
    });
  });

  it("disables diagnostics copy when no session is active", () => {
    const session = useSessionStore();
    session.currentSessionId = null;

    const wrapper = mountTimeline();
    const copyButton = wrapper.get('[data-test="trace-copy-diagnostics"]');

    expect(copyButton.attributes("disabled")).toBeDefined();
    expect(mockGeneratedCommands.exportSessionDiagnostics).not.toHaveBeenCalled();
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

  it("counts non-zero command exits as failed trace entries", async () => {
    traceState.entries = [
      makeTraceEntry("red-command", {
        title: "RED test command",
        status: "completed",
        exitCode: 1
      }),
      makeTraceEntry("green-command", {
        title: "GREEN test command",
        status: "completed",
        exitCode: 0
      }),
      makeTraceEntry("done", { title: "Done trace", status: "completed" })
    ];

    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();

    expect(wrapper.find('[data-test="trace-filter-failed"]').text()).toBe("Failed 1");
    expect(wrapper.find('[data-test="trace-filter-done"]').text()).toBe("Done 2");

    await wrapper.find('[data-test="trace-filter-failed"]').trigger("click");

    expect(wrapper.text()).toContain("RED test command");
    expect(wrapper.text()).not.toContain("GREEN test command");
    expect(wrapper.text()).not.toContain("Done trace");
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

  it("localizes trace filter controls", () => {
    traceState.entries = [makeTraceEntry("build", { title: "Build trace", status: "completed" })];

    const wrapper = mountTimeline("zh-CN");
    useTaskGraphStore().clearTaskGraph();

    const typeSelect = wrapper.get('[data-test="trace-kind-select"]');
    expect(typeSelect.attributes("aria-label")).toBe("追踪类型");
    expect(typeSelect.findAll("option").map((option) => option.text())).toEqual([
      "全部类型",
      "工具",
      "权限",
      "记忆"
    ]);

    const search = wrapper.get('[data-test="trace-search-input"]');
    expect(search.attributes("aria-label")).toBe("搜索追踪事件");
    expect(search.attributes("placeholder")).toBe("搜索追踪事件");
    expect(wrapper.find(".density-label").text()).toBe("详细程度：");
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
