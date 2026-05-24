// Per-tool icon registry for the chat stream.
//
// Maps Kairox built-in tool ids (shipped by `agent-tools`: `shell`,
// `fs.read`, `fs.write`, `fs.list`, `patch`, `search`) and the
// pseudo-tool ids emitted by the trace store (`task`, `user`,
// `assistant`, `context`, `model`, `memory.store`) to distinct emoji
// glyphs so the stream is visually scannable at a glance.
//
// Tool ids exposed by MCP servers use the `mcp::<server>::<tool>`
// shape (and a `mcp.<server>.<tool>` dot-variant in some configs). We
// match the server segment against a small per-server icon map for
// the common community servers (filesystem, github, slack,
// brave-search/web, memory, sqlite/postgres, git); anything else
// under `mcp::` / `mcp.` routes to a single generic MCP plug glyph.
// Anything outside both maps falls back to the generic-tool glyph
// already used by `TraceEntry.vue` (`🔧`) so the chat-stream surface
// stays consistent with the trace pane.
//
// Pure composable — no Pinia, no I/O — so it is trivially testable
// and safe to call from any component in any render context.

const BUILTIN_TOOL_ICONS: Readonly<Record<string, string>> = Object.freeze({
  // Built-in tools shipped by `agent-tools`.
  shell: "🖥️",
  "fs.read": "📖",
  "fs.write": "✏️",
  "fs.list": "📂",
  patch: "🩹",
  search: "🔎",
  // Pseudo-tool ids set by `stores/trace.ts` for non-tool events that
  // still render as a tool row in the chat stream / trace pane.
  task: "📋",
  user: "👤",
  assistant: "🤖",
  context: "📚",
  model: "✨",
  "memory.store": "💾"
});

// Per-server icons for common MCP servers, keyed by the server
// segment of an `mcp::<server>::<tool>` / `mcp.<server>.<tool>` id.
// Unknown servers fall through to `MCP_TOOL_ICON`.
const MCP_SERVER_ICONS: Readonly<Record<string, string>> = Object.freeze({
  filesystem: "📁",
  github: "🐙",
  slack: "💬",
  "brave-search": "🌐",
  web: "🌐",
  memory: "🧠",
  sqlite: "🗄️",
  postgres: "🗄️",
  git: "🌿"
});

const MCP_TOOL_ICON = "🔌";
const FALLBACK_TOOL_ICON = "🔧";

function isMcpToolId(toolId: string): boolean {
  return toolId.startsWith("mcp::") || toolId.startsWith("mcp.");
}

// Extracts the server segment from an MCP tool id, or `null` if the
// id is not an MCP id or has no server segment. Handles both the
// canonical `mcp::<server>::<tool>` shape and the `mcp.<server>.<tool>`
// dot-variant produced by some configs.
function mcpServerOf(toolId: string): string | null {
  if (toolId.startsWith("mcp::")) {
    const rest = toolId.slice("mcp::".length);
    if (rest.length === 0) return null;
    const sep = rest.indexOf("::");
    return sep >= 0 ? rest.slice(0, sep) : rest;
  }
  if (toolId.startsWith("mcp.")) {
    const rest = toolId.slice("mcp.".length);
    if (rest.length === 0) return null;
    const sep = rest.indexOf(".");
    return sep >= 0 ? rest.slice(0, sep) : rest;
  }
  return null;
}

export function useToolIcon() {
  function iconFor(toolId: string): string {
    const builtin = BUILTIN_TOOL_ICONS[toolId];
    if (builtin) return builtin;
    if (isMcpToolId(toolId)) {
      const server = mcpServerOf(toolId);
      if (server) {
        const serverIcon = MCP_SERVER_ICONS[server];
        if (serverIcon) return serverIcon;
      }
      return MCP_TOOL_ICON;
    }
    return FALLBACK_TOOL_ICON;
  }

  return { iconFor };
}
