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

  it("recognises the `mcp::server::tool` shape and routes to a per-server icon when mapped", () => {
    expect(iconFor("mcp::github::create_issue")).toBe("🐙");
  });

  it("recognises the dot-prefixed `mcp.<server>.<tool>` variant", () => {
    // Unmapped server falls through to the generic MCP plug glyph.
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
    const mcp = iconFor("mcp::unknownserver::y");
    const fallback = iconFor("totally_unknown_tool");
    expect(new Set(builtins).size).toBe(builtins.length);
    expect(builtins).not.toContain(mcp);
    expect(builtins).not.toContain(fallback);
    expect(mcp).not.toBe(fallback);
  });

  describe("MCP per-server icons", () => {
    it("returns the folder glyph for mcp.filesystem.* tools", () => {
      expect(iconFor("mcp.filesystem.read")).toBe("📁");
      expect(iconFor("mcp.filesystem.write")).toBe("📁");
      expect(iconFor("mcp.filesystem.list")).toBe("📁");
      expect(iconFor("mcp::filesystem::read_file")).toBe("📁");
    });

    it("returns the github glyph for mcp.github.* tools", () => {
      expect(iconFor("mcp.github.create_issue")).toBe("🐙");
      expect(iconFor("mcp::github::create_pull_request")).toBe("🐙");
    });

    it("returns the chat glyph for mcp.slack.* tools", () => {
      expect(iconFor("mcp.slack.post_message")).toBe("💬");
      expect(iconFor("mcp::slack::list_channels")).toBe("💬");
    });

    it("returns the globe glyph for mcp.brave-search.* and mcp.web.* tools", () => {
      expect(iconFor("mcp.brave-search.query")).toBe("🌐");
      expect(iconFor("mcp::brave-search::search")).toBe("🌐");
      expect(iconFor("mcp.web.fetch")).toBe("🌐");
      expect(iconFor("mcp::web::scrape")).toBe("🌐");
    });

    it("returns the brain glyph for mcp.memory.* tools", () => {
      expect(iconFor("mcp.memory.recall")).toBe("🧠");
      expect(iconFor("mcp::memory::store")).toBe("🧠");
    });

    it("returns the database glyph for mcp.sqlite.* and mcp.postgres.* tools", () => {
      expect(iconFor("mcp.sqlite.query")).toBe("🗄️");
      expect(iconFor("mcp::sqlite::execute")).toBe("🗄️");
      expect(iconFor("mcp.postgres.query")).toBe("🗄️");
      expect(iconFor("mcp::postgres::list_tables")).toBe("🗄️");
    });

    it("returns the branch glyph for mcp.git.* tools", () => {
      expect(iconFor("mcp.git.status")).toBe("🌿");
      expect(iconFor("mcp::git::log")).toBe("🌿");
    });

    it("still falls back to the generic MCP plug for unknown servers", () => {
      expect(iconFor("mcp.something_unmapped.foo")).toBe("🔌");
      expect(iconFor("mcp::something_unmapped::foo")).toBe("🔌");
    });

    it("returns the generic MCP plug when there is no server segment at all", () => {
      // Edge cases — malformed but defensible: no server segment.
      expect(iconFor("mcp::")).toBe("🔌");
      expect(iconFor("mcp.")).toBe("🔌");
    });
  });

  describe("trace-store pseudo-tools", () => {
    it("returns the task glyph for the `task` pseudo-tool", () => {
      expect(iconFor("task")).toBe("📋");
    });

    it("returns the user glyph for the `user` pseudo-tool", () => {
      expect(iconFor("user")).toBe("👤");
    });

    it("returns the assistant glyph for the `assistant` pseudo-tool", () => {
      expect(iconFor("assistant")).toBe("🤖");
    });

    it("returns the context glyph for the `context` pseudo-tool", () => {
      expect(iconFor("context")).toBe("📚");
    });

    it("returns the model glyph for the `model` pseudo-tool", () => {
      expect(iconFor("model")).toBe("✨");
    });

    it("returns the memory-store glyph for the `memory.store` pseudo-tool", () => {
      expect(iconFor("memory.store")).toBe("💾");
    });

    it("does not collide pseudo-tool glyphs with the fallback or generic MCP plug", () => {
      const pseudos = [
        iconFor("task"),
        iconFor("user"),
        iconFor("assistant"),
        iconFor("context"),
        iconFor("model"),
        iconFor("memory.store")
      ];
      const mcp = iconFor("mcp::unmapped::y");
      const fallback = iconFor("totally_unknown_tool");
      expect(new Set(pseudos).size).toBe(pseudos.length);
      expect(pseudos).not.toContain(mcp);
      expect(pseudos).not.toContain(fallback);
    });
  });
});
