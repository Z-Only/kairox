import { describe, it, expect } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders plain text as a paragraph", () => {
    const result = renderMarkdown("Hello world");
    expect(result).toContain("<p>");
    expect(result).toContain("Hello world");
  });

  it("highlights code blocks with a known language using hljs", () => {
    const result = renderMarkdown("```rust\nfn main() {}\n```");
    expect(result).toContain("hljs");
    // hljs wraps tokens in spans, so check for keyword span and the identifier
    expect(result).toContain("hljs-keyword");
    expect(result).toContain("fn");
    expect(result).toContain("main");
  });

  it("falls back to escaped HTML for code blocks with unknown language", () => {
    const result = renderMarkdown("```foobar\nsome code\n```");
    expect(result).toContain("<pre");
    expect(result).toContain("some code");
    // Should not have a language-specific hljs class like "language-foobar"
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
