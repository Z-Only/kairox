import { describe, it, expect, vi } from "vitest";
import type { ContextSource } from "@/types";

// `useContextFormatting` calls auto-imported `useI18n()` internally.
// Mock the vue-i18n module so the composable can be exercised outside
// a Vue component context — the mock `t` returns the raw key.
const mockT = vi.fn((key: string) => key);
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: mockT }) }));

import { useContextFormatting } from "./contextFormatting";

const ALL_SOURCES: ContextSource[] = [
  "system",
  "tool_definitions",
  "request",
  "memory",
  "workspace_retrieval",
  "history",
  "tool_result",
  "selected_file",
  "compaction_summary",
  "image"
];

describe("useContextFormatting", () => {
  const { formatTokens, formatSourceColor, formatSourceLabel, formatSourcePercent } =
    useContextFormatting();

  // ------- formatTokens -------
  describe("formatTokens", () => {
    it("returns raw number for values below 1000", () => {
      expect(formatTokens(0)).toBe("0");
      expect(formatTokens(1)).toBe("1");
      expect(formatTokens(999)).toBe("999");
    });

    it("formats values >= 1000 as X.Xk", () => {
      expect(formatTokens(1000)).toBe("1.0k");
      expect(formatTokens(1500)).toBe("1.5k");
      expect(formatTokens(10_000)).toBe("10.0k");
      expect(formatTokens(128_000)).toBe("128.0k");
    });

    it("rounds to one decimal place", () => {
      expect(formatTokens(1_050)).toBe("1.1k");
      expect(formatTokens(1_049)).toBe("1.0k");
      expect(formatTokens(99_999)).toBe("100.0k");
    });
  });

  // ------- formatSourceColor -------
  describe("formatSourceColor", () => {
    it.each<[ContextSource, string]>([
      ["system", "var(--src-system)"],
      ["project_instruction", "var(--src-project)"],
      ["tool_definitions", "var(--src-tools)"],
      ["memory", "var(--src-memory)"],
      ["workspace_retrieval", "var(--src-memory)"],
      ["git", "var(--src-git)"],
      ["history", "var(--src-history)"],
      ["tool_result", "var(--src-tool-result)"],
      ["selected_file", "var(--src-selected-file)"],
      ["compaction_summary", "var(--src-compaction-summary)"],
      ["skill", "var(--src-skill)"],
      ["request", "var(--src-request)"],
      ["image", "var(--src-image)"]
    ])("returns correct CSS variable for %s", (source, expected) => {
      expect(formatSourceColor(source)).toBe(expected);
    });

    it("returns fallback color for an unknown source", () => {
      expect(formatSourceColor("nonexistent")).toBe("var(--app-border-color, #d7d7d7)");
    });

    it("returns fallback color for an empty string", () => {
      expect(formatSourceColor("")).toBe("var(--app-border-color, #d7d7d7)");
    });

    it("covers every ContextSource variant", () => {
      for (const src of ALL_SOURCES) {
        const result = formatSourceColor(src);
        expect(result).toMatch(/^var\(--src-/);
      }
    });
  });

  // ------- formatSourceLabel -------
  describe("formatSourceLabel", () => {
    it.each<[ContextSource, string]>([
      ["system", "context.sourceSystem"],
      ["project_instruction", "context.sourceProjectInstruction"],
      ["tool_definitions", "context.sourceTools"],
      ["memory", "context.sourceMemory"],
      ["workspace_retrieval", "context.sourceWorkspaceRetrieval"],
      ["git", "context.sourceGit"],
      ["history", "context.sourceHistory"],
      ["tool_result", "context.sourceToolResult"],
      ["selected_file", "context.sourceSelectedFile"],
      ["compaction_summary", "context.sourceCompactionSummary"],
      ["skill", "context.sourceSkill"],
      ["request", "context.sourceRequest"],
      ["image", "context.sourceImage"]
    ])("returns the i18n key for %s", (source, expectedKey) => {
      expect(formatSourceLabel(source)).toBe(expectedKey);
    });

    it("returns the raw string for an unknown source", () => {
      expect(formatSourceLabel("custom_thing")).toBe("custom_thing");
    });

    it('returns "Unknown source" for an empty string', () => {
      expect(formatSourceLabel("")).toBe("Unknown source");
    });

    it("calls the i18n t() function for known sources", () => {
      mockT.mockClear();
      formatSourceLabel("system");
      expect(mockT).toHaveBeenCalledWith("context.sourceSystem");
    });
  });

  // ------- formatSourcePercent -------
  describe("formatSourcePercent", () => {
    it("returns 0 when budgetTokens is 0", () => {
      expect(formatSourcePercent(100, 0)).toBe(0);
    });

    it("returns 0 when budgetTokens is negative", () => {
      expect(formatSourcePercent(100, -10)).toBe(0);
    });

    it("calculates percentage correctly", () => {
      expect(formatSourcePercent(50, 100)).toBe(50);
      expect(formatSourcePercent(1, 3)).toBe(33);
      expect(formatSourcePercent(200, 200)).toBe(100);
    });

    it("rounds to the nearest integer", () => {
      // 1/3 = 33.333...% -> 33
      expect(formatSourcePercent(1, 3)).toBe(33);
      // 2/3 = 66.666...% -> 67
      expect(formatSourcePercent(2, 3)).toBe(67);
    });

    it("handles 0 tokens", () => {
      expect(formatSourcePercent(0, 1000)).toBe(0);
    });

    it("returns 0 for NaN / Infinity scenarios", () => {
      // 0/0 -> NaN -> not finite -> 0
      expect(formatSourcePercent(0, 0)).toBe(0);
    });
  });
});
