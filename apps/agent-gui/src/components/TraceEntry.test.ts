import { readFileSync } from "node:fs";
import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import TraceEntry from "./TraceEntry.vue";
import { traceState, clearTrace } from "../composables/useTraceStore";
import type { TraceEntryData } from "../types/trace";

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

const baseEntry: TraceEntryData = {
  id: "entry-1",
  kind: "tool",
  status: "completed",
  toolId: "shell_exec",
  title: "List files",
  startedAt: Date.now(),
  expanded: false
};

beforeEach(() => {
  setActivePinia(createPinia());
  clearTrace();
});

describe("TraceEntry", () => {
  it("hides detail when expanded is false and density is L2", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, expanded: false }, density: "L2" }
    });
    expect(wrapper.find(".entry-detail").exists()).toBe(false);
  });

  it("shows detail when expanded is true and density is L2", () => {
    traceState.entries.push({ ...baseEntry, expanded: true, input: "ls -la" });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L2" }
    });
    expect(wrapper.find(".entry-detail").exists()).toBe(true);
    expect(wrapper.find(".entry-detail").text()).toContain("ls -la");
  });

  it("shows context output detail when expanded in L1 density", () => {
    traceState.entries.push({
      ...baseEntry,
      toolId: "context",
      title: "Context assembled",
      outputPreview: "system:194, request:17",
      expanded: true
    });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L1" }
    });

    expect(wrapper.find(".entry-detail").exists()).toBe(true);
    expect(wrapper.find(".entry-detail").text()).toContain("system:194, request:17");
  });

  it("falls back to the context title when expanded context entries have no output payload", () => {
    traceState.entries.push({
      ...baseEntry,
      toolId: "context",
      title: "Context assembled (1511 / 181616 tokens)",
      expanded: true
    });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L1" }
    });

    expect(wrapper.find(".entry-detail").exists()).toBe(true);
    expect(wrapper.find(".entry-detail").text()).toContain("Context:");
    expect(wrapper.find(".entry-detail").text()).toContain("Context assembled");
  });

  it("shows task title detail when an expanded task entry has no input or output payload", () => {
    traceState.entries.push({
      ...baseEntry,
      toolId: "task",
      title: "请只回复 OK",
      expanded: true
    });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L2" }
    });

    expect(wrapper.find(".entry-detail").exists()).toBe(true);
    expect(wrapper.find(".entry-detail").text()).toContain("Task:");
    expect(wrapper.find(".entry-detail").text()).toContain("请只回复 OK");
  });

  it("toggles expanded on row click", async () => {
    traceState.entries.push({ ...baseEntry });
    const wrapper = mount(TraceEntry, {
      props: { entry: traceState.entries[0], density: "L2" }
    });
    expect(traceState.entries[0].expanded).toBe(false);
    await wrapper.find(".entry-row").trigger("click");
    expect(traceState.entries[0].expanded).toBe(true);
  });

  it("shows correct status icon for running", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "running" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("⏳");
  });

  it("shows correct status icon for completed", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "completed" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("✅");
  });

  it("shows failed treatment for completed commands with non-zero exit codes", () => {
    const wrapper = mount(TraceEntry, {
      props: {
        entry: {
          ...baseEntry,
          status: "completed",
          exitCode: 1,
          title: "bun test scripts/audit-eval-worktrees.test.mjs"
        },
        density: "L2"
      }
    });

    expect(wrapper.find(".trace-entry").classes()).toContain("trace-entry--failed");
    expect(wrapper.find(".entry-status").text()).toBe("❌");
    expect(wrapper.find(".entry-tool").text()).toContain(
      "bun test scripts/audit-eval-worktrees.test.mjs"
    );
  });

  it("keeps completed treatment for zero exit codes", () => {
    const wrapper = mount(TraceEntry, {
      props: {
        entry: { ...baseEntry, status: "completed", exitCode: 0, title: "cargo fmt --check" },
        density: "L2"
      }
    });

    expect(wrapper.find(".trace-entry").classes()).toContain("trace-entry--completed");
    expect(wrapper.find(".trace-entry").classes()).not.toContain("trace-entry--failed");
    expect(wrapper.find(".entry-status").text()).toBe("✅");
  });

  it("shows descriptive titles before raw tool ids in collapsed rows", () => {
    const wrapper = mount(TraceEntry, {
      props: {
        entry: { ...baseEntry, toolId: "shell.exec", title: "bun test scripts/foo.test.mjs" },
        density: "L2"
      }
    });

    expect(wrapper.find(".entry-tool .truncate").text()).toBe("bun test scripts/foo.test.mjs");
    expect(wrapper.find(".entry-tool-id").text()).toBe("shell.exec");
  });

  it("shows correct status icon for failed", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, status: "failed" }, density: "L2" }
    });
    expect(wrapper.find(".entry-status").text()).toBe("❌");
  });

  it("shows duration in seconds when durationMs is present", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, durationMs: 2500 }, density: "L2" }
    });
    const duration = wrapper.find(".entry-duration");
    expect(duration.text()).toBe("2.5s");
    expect(duration.attributes("style")).toContain("var(--app-text-color-3)");
  });

  it("applies kind CSS class for memory entries", () => {
    const wrapper = mount(TraceEntry, {
      props: { entry: { ...baseEntry, kind: "memory" }, density: "L2" }
    });
    expect(wrapper.find(".trace-entry").classes()).toContain("trace-entry--memory");
  });

  it("audit contrast tokens: keeps entry duration readable in dark theme", () => {
    const darkThemeProperties = getCustomProperties(themeCss, "html.dark");

    expect(
      getContrastRatio(
        darkThemeProperties["--app-text-color-3"],
        darkThemeProperties["--app-body-color"]
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
