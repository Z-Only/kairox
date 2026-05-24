// Per-tool icon registry for the chat stream.
//
// Maps Kairox built-in tool ids (shipped by `agent-tools`: `shell`,
// `fs.read`, `fs.write`, `fs.list`, `patch`, `search`) to distinct
// emoji glyphs so the stream is visually scannable at a glance.
// Tool ids exposed by MCP servers use the `mcp::<server>::<tool>`
// shape (and a `mcp.<server>.<tool>` dot-variant in some configs); we
// route both to a single MCP glyph. Anything else falls back to the
// generic-tool glyph already used by `TraceEntry.vue` (`🔧`) so the
// chat-stream surface stays consistent with the trace pane.
//
// Pure composable — no Pinia, no I/O — so it is trivially testable
// and safe to call from any component in any render context.

const BUILTIN_TOOL_ICONS: Readonly<Record<string, string>> = Object.freeze({
  shell: "🖥️",
  "fs.read": "📖",
  "fs.write": "✏️",
  "fs.list": "📂",
  patch: "🩹",
  search: "🔎"
});

const MCP_TOOL_ICON = "🔌";
const FALLBACK_TOOL_ICON = "🔧";

function isMcpToolId(toolId: string): boolean {
  return toolId.startsWith("mcp::") || toolId.startsWith("mcp.");
}

export function useToolIcon() {
  function iconFor(toolId: string): string {
    const builtin = BUILTIN_TOOL_ICONS[toolId];
    if (builtin) return builtin;
    if (isMcpToolId(toolId)) return MCP_TOOL_ICON;
    return FALLBACK_TOOL_ICON;
  }

  return { iconFor };
}
