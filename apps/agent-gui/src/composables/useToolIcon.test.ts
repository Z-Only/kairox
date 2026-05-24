import { describe, it, expect } from "vitest";
import { useToolIcon } from "./useToolIcon";

describe("useToolIcon", () => {
  const { iconFor } = useToolIcon();

  it("returns the shell glyph for the shell tool id", () => {
    expect(iconFor("shell")).toBe("🖥️");
  });

  it("returns distinct glyphs for each built-in fs.* tool", () => {
    const read = iconFor("fs.read");
    const write = iconFor("fs.write");
    const list = iconFor("fs.list");
    expect(read).toBe("📖");
    expect(write).toBe("✏️");
    expect(list).toBe("📂");
    // Distinct from each other
    expect(new Set([read, write, list]).size).toBe(3);
  });

  it("returns the patch glyph for patch", () => {
    expect(iconFor("patch")).toBe("🩹");
  });

  it("returns the search glyph for search", () => {
    expect(iconFor("search")).toBe("🔎");
  });

  it("returns the MCP glyph for `mcp::server::tool` ids", () => {
    expect(iconFor("mcp::github::create_issue")).toBe("🔌");
  });

  it("returns the MCP glyph for the dot-prefixed `mcp.foo.bar` variant", () => {
    expect(iconFor("mcp.foo.bar")).toBe("🔌");
  });

  it("returns the generic fallback for unknown tool ids", () => {
    expect(iconFor("totally_unknown_tool")).toBe("🔧");
  });

  it("does not collide built-in glyphs with the MCP or fallback glyphs", () => {
    const builtins = [
      iconFor("shell"),
      iconFor("fs.read"),
      iconFor("fs.write"),
      iconFor("fs.list"),
      iconFor("patch"),
      iconFor("search")
    ];
    const mcp = iconFor("mcp::x::y");
    const fallback = iconFor("totally_unknown_tool");
    expect(new Set(builtins).size).toBe(builtins.length);
    expect(builtins).not.toContain(mcp);
    expect(builtins).not.toContain(fallback);
    expect(mcp).not.toBe(fallback);
  });
});
