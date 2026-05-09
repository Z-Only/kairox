import { readFileSync } from "node:fs";
import { describe, it, expect, beforeEach, vi } from "vitest";
import TraceTimeline from "./TraceTimeline.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);
const componentsCss = readFileSync("src/styles/components.css", "utf8");
const themeCss = readFileSync("src/styles/theme.css", "utf8");

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
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        }
      }
    }
  }).wrapper;
}

beforeEach(() => {
  clearTrace();
  // MemoryBrowser calls `invoke('query_memories', ...)` on mount and
  // assigns the result to `memories.value`. Without a default resolved
  // value, vitest mocks return `undefined`, which makes `memories.length`
  // throw inside the template render. Supply a stable empty-array
  // default so any invoke call this test file does not override stays
  // well-typed.
  mockedInvoke.mockResolvedValue([]);
  // The Pinia-bound `useTaskGraphStore().clearTaskGraph()` reset is now
  // done *after* mount inside each test (see `mountTimeline()` helper
  // calls below); the shared helper installs a fresh Pinia per mount so
  // there is no module-level state to reset before mount.
});

describe("TraceTimeline", () => {
  it("shows Trace tab as active by default", () => {
    const wrapper = mountTimeline();
    useTaskGraphStore().clearTaskGraph();
    const buttons = wrapper.findAll(".tab-group button");
    expect(buttons[0].classes()).toEqual(expect.arrayContaining(["active", "btn-primary"]));
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

    expect(componentsCss).toContain("color: var(--app-primary-contrast-color);");
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
});
