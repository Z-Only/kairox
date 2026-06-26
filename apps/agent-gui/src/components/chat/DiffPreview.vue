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
  showViewToggle?: boolean;
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

type DiffViewMode = "inline" | "split";

type SplitDiffRow =
  | { type: "meta"; key: string; line: ClassifiedLine }
  | { type: "pair"; key: string; oldLine: ClassifiedLine | null; newLine: ClassifiedLine | null }
  | { type: "collapsed-context"; key: string; count: number };

const viewMode = ref<DiffViewMode>("inline");

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

const splitRows = computed<SplitDiffRow[]>(() => {
  const rows = renderedRows.value;
  const split: SplitDiffRow[] = [];
  let idx = 0;

  const lineAt = (index: number): ClassifiedLine | null => {
    const row = rows[index];
    return row?.type === "line" ? row.line : null;
  };

  while (idx < rows.length) {
    const row = rows[idx];
    if (row.type === "collapsed-context") {
      split.push({ type: "collapsed-context", key: `split-${row.key}`, count: row.count });
      idx += 1;
      continue;
    }

    const line = row.line;
    if (line.kind === "removed") {
      const removed: ClassifiedLine[] = [];
      const added: ClassifiedLine[] = [];
      while (lineAt(idx)?.kind === "removed") {
        removed.push(lineAt(idx)!);
        idx += 1;
      }
      while (lineAt(idx)?.kind === "added") {
        added.push(lineAt(idx)!);
        idx += 1;
      }
      const pairCount = Math.max(removed.length, added.length);
      for (let pairIdx = 0; pairIdx < pairCount; pairIdx += 1) {
        split.push({
          type: "pair",
          key: `split-pair-${idx}-${pairIdx}`,
          oldLine: removed[pairIdx] ?? null,
          newLine: added[pairIdx] ?? null
        });
      }
      continue;
    }

    if (line.kind === "added") {
      split.push({ type: "pair", key: `split-${row.key}`, oldLine: null, newLine: line });
    } else if (line.kind === "context") {
      split.push({ type: "pair", key: `split-${row.key}`, oldLine: line, newLine: line });
    } else {
      split.push({ type: "meta", key: `split-${row.key}`, line });
    }
    idx += 1;
  }

  return split;
});

const isSplitView = computed(() => props.showViewToggle && viewMode.value === "split");
const ariaLabel = computed(() => t("chatStream.toolCall.diffPreview"));

function setViewMode(mode: DiffViewMode): void {
  viewMode.value = mode;
}

function showUnchangedLabel(count: number): string {
  return t(count === 1 ? "chat.gitReview.showUnchangedLine" : "chat.gitReview.showUnchangedLines", {
    count
  });
}
</script>

<template>
  <div
    v-if="props.showViewToggle"
    class="diff-view-toggle"
    role="group"
    :aria-label="t('chat.gitReview.diffViewMode')"
  >
    <button
      type="button"
      class="diff-view-toggle__button"
      data-test="diff-view-inline"
      :aria-pressed="viewMode === 'inline'"
      @click="setViewMode('inline')"
    >
      {{ t("chat.gitReview.diffViewInline") }}
    </button>
    <button
      type="button"
      class="diff-view-toggle__button"
      data-test="diff-view-split"
      :aria-pressed="viewMode === 'split'"
      @click="setViewMode('split')"
    >
      {{ t("chat.gitReview.diffViewSplit") }}
    </button>
  </div>
  <pre
    v-if="!isSplitView && !props.collapseUnmodified"
    class="diff-preview"
    data-test="diff-preview"
    :aria-label="ariaLabel"
  ><span
      v-for="(line, idx) in lines"
      :key="idx"
      :class="['diff-line', `diff-line--${line.kind}`]"
      data-test="diff-line"
    >{{ line.text }}<br /></span></pre>
  <div
    v-else-if="!isSplitView"
    class="diff-preview"
    data-test="diff-preview"
    role="region"
    :aria-label="ariaLabel"
  >
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
  <div
    v-else
    class="diff-preview diff-preview--split"
    data-test="diff-preview"
    role="region"
    :aria-label="ariaLabel"
  >
    <span class="diff-split-heading" data-test="diff-split-old-header">{{
      t("chat.gitReview.diffViewOld")
    }}</span>
    <span class="diff-split-heading" data-test="diff-split-new-header">{{
      t("chat.gitReview.diffViewNew")
    }}</span>
    <template v-for="row in splitRows" :key="row.key">
      <span
        v-if="row.type === 'meta'"
        :class="['diff-split-meta', `diff-line--${row.line.kind}`]"
        data-test="diff-split-meta"
        >{{ row.line.text }}</span
      >
      <span v-else-if="row.type === 'pair'" class="diff-split-row" data-test="diff-split-row">
        <span
          :class="[
            'diff-split-cell',
            'diff-split-cell--old',
            row.oldLine ? `diff-line--${row.oldLine.kind}` : 'diff-split-cell--empty'
          ]"
          data-test="diff-split-old"
          >{{ row.oldLine?.text ?? "" }}</span
        >
        <span
          :class="[
            'diff-split-cell',
            'diff-split-cell--new',
            row.newLine ? `diff-line--${row.newLine.kind}` : 'diff-split-cell--empty'
          ]"
          data-test="diff-split-new"
          >{{ row.newLine?.text ?? "" }}</span
        >
      </span>
      <button
        v-else
        type="button"
        class="diff-context-toggle diff-split-context-toggle"
        data-test="diff-collapsed-context"
        @click="emit('toggle-unmodified')"
      >
        {{ showUnchangedLabel(row.count) }}
      </button>
    </template>
  </div>
</template>

<style scoped>
.diff-view-toggle {
  display: inline-flex;
  margin: 2px 0 4px;
  overflow: hidden;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-panel-color);
}
.diff-view-toggle__button {
  min-height: 24px;
  padding: 2px 8px;
  border: 0;
  border-right: 1px solid var(--app-border-color);
  background: transparent;
  color: var(--app-text-color-2);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
}
.diff-view-toggle__button:last-child {
  border-right: 0;
}
.diff-view-toggle__button[aria-pressed="true"] {
  background: color-mix(in srgb, var(--app-primary-color) 14%, transparent);
  color: var(--app-primary-color);
  font-weight: 600;
}
.diff-view-toggle__button:hover,
.diff-view-toggle__button:focus-visible {
  outline: none;
  color: var(--app-text-color);
}
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
.diff-preview--split {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  padding: 0;
}
.diff-split-heading,
.diff-split-cell,
.diff-split-meta {
  min-width: 0;
  padding: 2px 8px;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
.diff-split-heading {
  color: var(--app-text-color-3);
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  border-bottom: 1px solid var(--app-border-color);
  background: color-mix(in srgb, var(--app-panel-color) 88%, var(--app-code-bg));
}
.diff-split-row {
  display: contents;
}
.diff-split-cell {
  border-top: 1px solid color-mix(in srgb, var(--app-border-color) 60%, transparent);
}
.diff-split-cell--old {
  border-right: 1px solid var(--app-border-color);
}
.diff-split-cell--empty {
  background: transparent;
}
.diff-split-meta {
  grid-column: 1 / -1;
  border-top: 1px solid color-mix(in srgb, var(--app-border-color) 60%, transparent);
}
.diff-split-context-toggle {
  grid-column: 1 / -1;
  width: auto;
  padding: 2px 8px;
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
