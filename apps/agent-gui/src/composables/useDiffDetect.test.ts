import { describe, it, expect } from "vitest";
import { isDiffShaped, useDiffDetect } from "./useDiffDetect";

describe("useDiffDetect.isDiffShaped", () => {
  it("returns false for an empty string", () => {
    expect(isDiffShaped("")).toBe(false);
  });

  it("returns false for plain prose without diff markers", () => {
    expect(isDiffShaped("hello\nworld")).toBe(false);
  });

  it("returns true when a unified-diff header is present", () => {
    const diff = "--- a/foo\n+++ b/foo\n@@ -1 +1 @@\n-foo\n+bar";
    expect(isDiffShaped(diff)).toBe(true);
  });

  it("returns true when two or more `+`/`-` lines appear even without a header", () => {
    const diff = "+ added\n- removed\n+ added2";
    expect(isDiffShaped(diff)).toBe(true);
  });

  it("returns false when the leading `+` lines look numeric (not diff markers)", () => {
    expect(isDiffShaped("+1 reason: ...\n+2 cause: ...")).toBe(false);
  });

  it("returns true for two minimal `+`/`-` lines (space after sigil)", () => {
    expect(isDiffShaped("+ a\n- b")).toBe(true);
  });

  it("returns false for a single `+` line — needs at least two markers", () => {
    expect(isDiffShaped("+ only one")).toBe(false);
  });

  it("returns false for markdown-style headings starting with `--` and `++`", () => {
    expect(isDiffShaped("-- heading\n++ another")).toBe(false);
  });

  it("returns true for a `@@` hunk header alone (canonical unified diff signal)", () => {
    expect(isDiffShaped("@@ -1,3 +1,3 @@\n context\n more")).toBe(true);
  });

  it("returns false for non-string inputs treated as empty", () => {
    // Defensive: callers may pass undefined-ish — guard against runtime surprises.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect(isDiffShaped(undefined as any)).toBe(false);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect(isDiffShaped(null as any)).toBe(false);
  });

  it("returns false for single-char +/- lines (ambiguous)", () => {
    expect(isDiffShaped("+\n-")).toBe(false);
  });

  it("returns true for `--- a/` header alone without full diff body", () => {
    expect(isDiffShaped("--- a/some/file.ts\ncontext line")).toBe(true);
  });

  it("returns true for `+++ b/` header alone", () => {
    expect(isDiffShaped("+++ b/another/file.rs\ncontext")).toBe(true);
  });
});

describe("useDiffDetect composable", () => {
  it("returns an object with isDiffShaped function", () => {
    const result = useDiffDetect();
    expect(result).toHaveProperty("isDiffShaped");
    expect(typeof result.isDiffShaped).toBe("function");
  });

  it("exposed isDiffShaped is the same function as the named export", () => {
    const { isDiffShaped: composableIsDiffShaped } = useDiffDetect();
    expect(composableIsDiffShaped).toBe(isDiffShaped);
  });

  it("works correctly when called through the composable", () => {
    const { isDiffShaped: detect } = useDiffDetect();
    expect(detect("--- a/foo\n+++ b/foo\n@@ -1 +1 @@\n-old\n+new")).toBe(true);
    expect(detect("plain text")).toBe(false);
  });
});
