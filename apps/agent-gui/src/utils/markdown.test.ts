import { describe, it, expect } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders plain text as a paragraph", () => {
    const result = renderMarkdown("Hello world");
    expect(result).toContain("<p>");
    expect(result).toContain("Hello world");
  });

  it.each([
    ["rust", "fn main() {}", "hljs-keyword"],
    ["typescript", "const value: string = 'ok';", "hljs-keyword"],
    ["json", '{"ok": true}', "hljs-attr"],
    ["bash", "echo hello", "hljs-built_in"]
  ])("highlights registered %s code blocks", (language, code, expectedClass) => {
    const result = renderMarkdown(`\`\`\`${language}\n${code}\n\`\`\``);
    expect(result).toContain("hljs");
    expect(result).toContain(expectedClass);
  });

  it("falls back to escaped HTML for code blocks with unknown language", () => {
    const result = renderMarkdown("```foobar\n<script>bad()</script>\n```");
    expect(result).toContain("<pre");
    expect(result).toContain("&lt;script&gt;bad()&lt;/script&gt;");
    expect(result).not.toContain("language-foobar");
  });

  it("renders inline code", () => {
    const result = renderMarkdown("Use `cargo test` to run");
    expect(result).toContain("<code>");
    expect(result).toContain("cargo test");
  });

  it("escapes HTML to prevent XSS", () => {
    const result = renderMarkdown('<script>alert("xss")</script>');
    expect(result).not.toContain("<script>");
    expect(result).toContain("&lt;script&gt;");
  });
});
