import { describe, it, expect } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import DiffPreview from "@/components/chat/DiffPreview.vue";

function mountDiff(props: Record<string, unknown>) {
  return mountWithPlugins(DiffPreview, {
    mount: { props },
    reusePinia: true
  }).wrapper;
}

describe("DiffPreview", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders a `<pre>` with the i18n aria-label", () => {
    const wrapper = mountDiff({ text: "hello\nworld" });
    const pre = wrapper.find("pre");
    expect(pre.exists()).toBe(true);
    expect(pre.attributes("aria-label")).toBe("Diff preview");
  });

  it("treats every line as a default line when text is plain prose", () => {
    const wrapper = mountDiff({ text: "alpha\nbeta\ngamma" });
    const lines = wrapper.findAll('[data-test="diff-line"]');
    expect(lines).toHaveLength(3);
    for (const line of lines) {
      expect(line.classes()).toContain("diff-line--default");
    }
  });

  it("classifies unified-diff lines with the right modifier class", () => {
    const diff = "--- a/foo\n+++ b/foo\n@@ -1,2 +1,2 @@\n-foo\n+bar\n unchanged";
    const wrapper = mountDiff({ text: diff });
    const lines = wrapper.findAll('[data-test="diff-line"]');
    // 6 source lines → 6 rendered lines
    expect(lines).toHaveLength(6);

    // --- a/foo
    expect(lines[0].classes()).toContain("diff-line--file-old");
    // +++ b/foo
    expect(lines[1].classes()).toContain("diff-line--file-new");
    // @@ hunk header
    expect(lines[2].classes()).toContain("diff-line--hunk");
    // -foo
    expect(lines[3].classes()).toContain("diff-line--removed");
    // +bar
    expect(lines[4].classes()).toContain("diff-line--added");
    // ` unchanged` — unified diff context line
    expect(lines[5].classes()).toContain("diff-line--context");
  });

  it("treats mixed garbage lines as default when they do not match any sigil", () => {
    const wrapper = mountDiff({
      text: "random noise\n>> arrow line\n??? unknown"
    });
    const lines = wrapper.findAll('[data-test="diff-line"]');
    expect(lines).toHaveLength(3);
    for (const line of lines) {
      expect(line.classes()).toContain("diff-line--default");
    }
  });

  it("renders nothing when the text prop is empty", () => {
    const wrapper = mountDiff({ text: "" });
    expect(wrapper.findAll('[data-test="diff-line"]')).toHaveLength(0);
  });

  it("preserves the literal source characters in each rendered line", () => {
    const diff = "+ alpha\n- beta\n@@ middle @@";
    const wrapper = mountDiff({ text: diff });
    const lines = wrapper.findAll('[data-test="diff-line"]');
    expect(lines[0].text()).toBe("+ alpha");
    expect(lines[1].text()).toBe("- beta");
    expect(lines[2].text()).toBe("@@ middle @@");
  });

  it("collapses unchanged context lines by default and expands them on request", async () => {
    const wrapper = mountDiff({
      text: "--- a/foo\n+++ b/foo\n@@ -1,4 +1,4 @@\n keep one\n-old\n+new\n keep two",
      collapseUnmodified: true
    });

    expect(wrapper.text()).toContain("-old");
    expect(wrapper.text()).toContain("+new");
    expect(wrapper.text()).not.toContain("keep one");
    expect(wrapper.text()).not.toContain("keep two");

    const collapsedRows = wrapper.findAll('[data-test="diff-collapsed-context"]');
    expect(collapsedRows).toHaveLength(2);
    const collapsed = collapsedRows[0];
    expect(collapsed.text()).toBe("Show 1 unchanged line");

    await collapsed.trigger("click");
    expect(wrapper.emitted("toggle-unmodified")).toHaveLength(1);

    await wrapper.setProps({ unmodifiedExpanded: true });
    expect(wrapper.text()).toContain("keep one");
    expect(wrapper.text()).toContain("keep two");
  });

  it("does NOT colorize markdown-style headings starting with `--` or `++`", () => {
    const wrapper = mountDiff({
      text: "-- markdown heading\n++ another heading"
    });
    const lines = wrapper.findAll('[data-test="diff-line"]');
    expect(lines).toHaveLength(2);
    // First line `--` — without a path after the dashes, NOT file-old.
    expect(lines[0].classes()).toContain("diff-line--default");
    expect(lines[1].classes()).toContain("diff-line--default");
  });
});
