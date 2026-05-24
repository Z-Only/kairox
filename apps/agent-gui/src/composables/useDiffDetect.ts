// Lightweight detector for unified-diff-shaped text.
//
// Used by `ChatToolCallItem.vue` to decide whether a tool's
// `output_preview` should be rendered with the colored monospace
// `DiffPreview` component or left in the existing plain `<pre>`.
//
// Heuristic (intentionally narrow — we only want to upgrade output
// that is clearly diff-shaped, never to mis-classify ordinary logs):
//
//  - Canonical unified-diff headers (`--- a/...`, `+++ b/...`, `@@ ...`)
//    are an immediate strong signal.
//  - Otherwise: two or more lines whose first character is `+` or `-`
//    AND whose second character looks like a diff marker (space, tab,
//    end-of-line, or alphabetic content) — i.e. NOT a numeric digit
//    (`+1`, `-2.5`) and NOT a doubled sigil (`++`, `--`) which is how
//    markdown headings / divider lines look.
//
// Pure helper — no Vue reactivity, no I/O — so it's trivially testable
// and safe to call from any context.

const HEADER_PATTERNS: readonly RegExp[] = [
  /^---\s+\S/m, // `--- a/path`
  /^\+\+\+\s+\S/m, // `+++ b/path`
  /^@@\s/m // `@@ -1,3 +1,3 @@`
];

function looksLikeDiffMarkerLine(line: string): boolean {
  if (line.length === 0) return false;
  const first = line.charCodeAt(0); // 0x2B = '+', 0x2D = '-'
  if (first !== 0x2b && first !== 0x2d) return false;
  // Single-char line ("+" or "-") is too ambiguous; require a second char.
  if (line.length === 1) return false;
  const second = line[1];
  // Reject doubled sigils (`++`, `--`) — usually markdown / dividers.
  if (second === "+" || second === "-") return false;
  // Reject numeric-looking content (`+1`, `-2.5`) — usually list / data.
  if (second >= "0" && second <= "9") return false;
  return true;
}

export function isDiffShaped(text: string): boolean {
  if (typeof text !== "string" || text.length === 0) return false;

  // Canonical headers are a single strong signal.
  for (const pattern of HEADER_PATTERNS) {
    if (pattern.test(text)) return true;
  }

  // Otherwise require >= 2 lines that look like diff +/- markers.
  let markerLines = 0;
  for (const line of text.split("\n")) {
    if (looksLikeDiffMarkerLine(line)) {
      markerLines += 1;
      if (markerLines >= 2) return true;
    }
  }
  return false;
}

export function useDiffDetect() {
  return { isDiffShaped };
}
