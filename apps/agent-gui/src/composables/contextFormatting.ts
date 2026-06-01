import type { ContextSource } from "@/types";

const sourceColorVar: Record<ContextSource, string> = {
  system: "var(--src-system)",
  tool_definitions: "var(--src-tools)",
  memory: "var(--src-memory)",
  history: "var(--src-history)",
  tool_result: "var(--src-tool-result)",
  selected_file: "var(--src-selected-file)",
  compaction_summary: "var(--src-compaction-summary)",
  request: "var(--src-request)",
  image: "var(--src-image)"
};

const sourceLabelKey: Record<ContextSource, string> = {
  system: "context.sourceSystem",
  tool_definitions: "context.sourceTools",
  memory: "context.sourceMemory",
  history: "context.sourceHistory",
  tool_result: "context.sourceToolResult",
  selected_file: "context.sourceSelectedFile",
  compaction_summary: "context.sourceCompactionSummary",
  request: "context.sourceRequest",
  image: "context.sourceImage"
};

export function useContextFormatting() {
  const { t } = useI18n();

  function formatTokens(n: number): string {
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
    return String(n);
  }

  function formatSourceColor(source: string): string {
    if (Object.prototype.hasOwnProperty.call(sourceColorVar, source)) {
      return sourceColorVar[source as ContextSource];
    }
    return "var(--app-border-color, #d7d7d7)";
  }

  function formatSourceLabel(source: string): string {
    if (Object.prototype.hasOwnProperty.call(sourceLabelKey, source)) {
      return t(sourceLabelKey[source as ContextSource]);
    }
    return source || "Unknown source";
  }

  function formatSourcePercent(tokens: number, budgetTokens: number): number {
    if (budgetTokens <= 0) return 0;
    const percentage = (tokens / budgetTokens) * 100;
    if (!Number.isFinite(percentage)) return 0;
    return Math.round(percentage);
  }

  return { formatTokens, formatSourceColor, formatSourceLabel, formatSourcePercent };
}
