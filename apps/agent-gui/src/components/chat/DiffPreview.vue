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
}

const props = defineProps<DiffPreviewProps>();

const { t } = useI18n();

type DiffLineKind = "file-old" | "file-new" | "hunk" | "added" | "removed" | "default";

interface ClassifiedLine {
  kind: DiffLineKind;
  text: string;
}

function classify(line: string): DiffLineKind {
  // File headers require a non-empty path component after the sigil so
  // markdown headings like "-- foo" or "++ bar" do NOT colorize.
  if (/^---\s+\S/.test(line)) return "file-old";
  if (/^\+\+\+\s+\S/.test(line)) return "file-new";
  if (line.startsWith("@@")) return "hunk";
  // Added / removed lines: must NOT be a doubled sigil header.
  if (line.startsWith("+") && !line.startsWith("++")) return "added";
  if (line.startsWith("-") && !line.startsWith("--")) return "removed";
  return "default";
}

const lines = computed<ClassifiedLine[]>(() => {
  if (!props.text) return [];
  return props.text.split("\n").map((line) => ({ kind: classify(line), text: line }));
});

const ariaLabel = computed(() => t("chatStream.toolCall.diffPreview"));
</script>

<template>
  <pre class="diff-preview" data-test="diff-preview" :aria-label="ariaLabel"><span
      v-for="(line, idx) in lines"
      :key="idx"
      :class="['diff-line', `diff-line--${line.kind}`]"
      data-test="diff-line"
    >{{ line.text }}<br /></span></pre>
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
</style>
