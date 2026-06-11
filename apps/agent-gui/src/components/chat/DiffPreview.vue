<script setup lang="ts">
// Minimal colored renderer for unified-diff-shaped text.
//
// Used by `ChatToolCallItem.vue` when `useDiffDetect.isDiffShaped()`
// matches a tool's `output_preview`. Splits the input into lines and
// classifies each one into a small set of buckets (file header,
// hunk header, added, removed, default) so per-line scoped styles
// can color them without pulling in a heavy syntax-highlighter.
//
// Non-diff output paths continue to use the plain `<pre>` in
// `ChatToolCallItem.vue`; this component is intentionally tiny and
// has no opinions about whether the text is "really" a diff —
// callers decide via the detector before mounting.

interface DiffPreviewProps {
  text: string;
  collapseUnmodified?: boolean;
  unmodifiedExpanded?: boolean;
}

const props = defineProps<DiffPreviewProps>();
const emit = defineEmits<{
  "toggle-unmodified": [];
}>();

const { t } = useI18n();

type DiffLineKind = "file-old" | "file-new" | "hunk" | "added" | "removed" | "context" | "default";

interface ClassifiedLine {
  kind: DiffLineKind;
  text: string;
}

type RenderedDiffRow =
  | { type: "line"; key: string; line: ClassifiedLine }
  | { type: "collapsed-context"; key: string; count: number };

function classify(line: string): DiffLineKind {
  // File headers require a non-empty path component after the sigil so
  // markdown headings like "-- foo" or "++ bar" do NOT colorize.
  if (/^---\s+\S/.test(line)) return "file-old";
  if (/^\+\+\+\s+\S/.test(line)) return "file-new";
  if (line.startsWith("@@")) return "hunk";
  // Added / removed lines: must NOT be a doubled sigil header.
  if (line.startsWith("+") && !line.startsWith("++")) return "added";
  if (line.startsWith("-") && !line.startsWith("--")) return "removed";
  if (line.startsWith(" ")) return "context";
  return "default";
}

const lines = computed<ClassifiedLine[]>(() => {
  if (!props.text) return [];
  return props.text.split("\n").map((line) => ({ kind: classify(line), text: line }));
});

const hasContextLines = computed(() => lines.value.some((line) => line.kind === "context"));

const renderedRows = computed<RenderedDiffRow[]>(() => {
  if (!props.collapseUnmodified || props.unmodifiedExpanded) {
    return lines.value.map((line, idx) => ({
      type: "line",
      key: `line-${idx}`,
      line
    }));
  }

  const rows: RenderedDiffRow[] = [];
  let collapsedCount = 0;
  let collapsedIndex = 0;

  const flushCollapsedContext = () => {
    if (!collapsedCount) return;
    rows.push({
      type: "collapsed-context",
      key: `collapsed-context-${collapsedIndex}`,
      count: collapsedCount
    });
    collapsedIndex += 1;
    collapsedCount = 0;
  };

  lines.value.forEach((line, idx) => {
    if (line.kind === "context") {
      collapsedCount += 1;
      return;
    }
    flushCollapsedContext();
    rows.push({
      type: "line",
      key: `line-${idx}`,
      line
    });
  });
  flushCollapsedContext();

  return rows;
});

const ariaLabel = computed(() => t("chatStream.toolCall.diffPreview"));

function showUnchangedLabel(count: number): string {
  return t(count === 1 ? "chat.gitReview.showUnchangedLine" : "chat.gitReview.showUnchangedLines", {
    count
  });
}
</script>

<template>
  <pre
    v-if="!props.collapseUnmodified"
    class="diff-preview"
    data-test="diff-preview"
    :aria-label="ariaLabel"
  ><span
      v-for="(line, idx) in lines"
      :key="idx"
      :class="['diff-line', `diff-line--${line.kind}`]"
      data-test="diff-line"
    >{{ line.text }}<br /></span></pre>
  <div v-else class="diff-preview" data-test="diff-preview" role="region" :aria-label="ariaLabel">
    <button
      v-if="props.unmodifiedExpanded && hasContextLines"
      type="button"
      class="diff-context-toggle"
      data-test="diff-expanded-context"
      @click="emit('toggle-unmodified')"
    >
      {{ t("chat.gitReview.hideUnchangedLines") }}
    </button>
    <template v-for="row in renderedRows" :key="row.key">
      <span
        v-if="row.type === 'line'"
        :class="['diff-line', `diff-line--${row.line.kind}`]"
        data-test="diff-line"
        >{{ row.line.text }}<br
      /></span>
      <button
        v-else
        type="button"
        class="diff-context-toggle"
        data-test="diff-collapsed-context"
        @click="emit('toggle-unmodified')"
      >
        {{ showUnchangedLabel(row.count) }}
      </button>
    </template>
  </div>
</template>

<style scoped>
.diff-preview {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: var(--app-code-bg);
  color: var(--app-text-color);
  border-radius: 4px;
  font-family:
    ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.diff-line {
  display: block;
}
.diff-line--default {
  color: var(--app-text-color);
}
.diff-line--context {
  color: var(--app-text-color-2);
}
.diff-line--added {
  color: var(--app-success-color, #2ea043);
  background: color-mix(in srgb, var(--app-success-color, #2ea043) 10%, transparent);
}
.diff-line--removed {
  color: var(--app-error-color, #d03050);
  background: color-mix(in srgb, var(--app-error-color, #d03050) 10%, transparent);
}
.diff-line--hunk {
  color: var(--app-info-color, #0ea5e9);
  font-weight: 600;
}
.diff-line--file-old {
  color: var(--app-error-color, #d03050);
  font-weight: 600;
}
.diff-line--file-new {
  color: var(--app-success-color, #2ea043);
  font-weight: 600;
}
/* The `<br>` lets each line wrap visually when long; hide its inline box
   so it does not introduce extra vertical whitespace. */
.diff-line br {
  display: none;
}
.diff-context-toggle {
  display: block;
  width: 100%;
  margin: 2px 0;
  padding: 2px 0;
  border: 0;
  background: transparent;
  color: var(--app-text-color-3);
  cursor: pointer;
  font: inherit;
  text-align: left;
}
.diff-context-toggle:hover,
.diff-context-toggle:focus-visible {
  outline: none;
  color: var(--app-text-color);
  text-decoration: underline;
}
</style>
