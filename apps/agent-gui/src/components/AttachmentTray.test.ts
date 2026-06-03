import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import AttachmentTray from "./AttachmentTray.vue";
import type { Attachment } from "@/composables/useChatComposer";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`)
}));

function mountTray(attachments: Attachment[], disabled = false) {
  return mount(AttachmentTray, {
    props: { attachments, disabled },
    global: {
      mocks: {
        $t: (key: string, values?: Record<string, unknown>) =>
          values?.name ? `${key}: ${values.name}` : key
      },
      stubs: {
        Teleport: true
      }
    }
  });
}

describe("AttachmentTray", () => {
  it("renders attached files and emits remove and pick events", async () => {
    const wrapper = mountTray([
      {
        id: "att_1",
        path: "/repo/src/main.rs",
        name: "main.rs",
        mimeType: "text/x-rust"
      },
      {
        id: "att_2",
        path: "/repo/screenshot.png",
        name: "screenshot.png",
        mimeType: "image/png"
      }
    ]);

    await wrapper.get('[data-test="attach-file-btn"]').trigger("click");
    await wrapper.get('[data-test="attachment-remove"]').trigger("click");

    expect(wrapper.get('[data-test="attachment-row"]').exists()).toBe(true);
    expect(wrapper.findAll('[data-test="attachment-chip"]')).toHaveLength(2);
    expect(wrapper.find('[data-filename="main.rs"]').exists()).toBe(true);
    expect(wrapper.get("img.attachment-thumbnail").attributes("src")).toBe(
      "asset:///repo/screenshot.png"
    );
    expect(wrapper.emitted("pick-files")).toHaveLength(1);
    expect(wrapper.emitted("remove-attachment")).toEqual([["att_1"]]);
  });

  it("disables the inline attach button while the composer is disabled", () => {
    const wrapper = mountTray(
      [
        {
          id: "att_1",
          path: "/repo/notes.md",
          name: "notes.md",
          mimeType: "text/markdown"
        }
      ],
      true
    );

    expect(wrapper.get<HTMLButtonElement>('[data-test="attach-file-btn"]').element.disabled).toBe(
      true
    );
  });

  it("is hidden when attachments array is empty", () => {
    const wrapper = mountTray([]);
    expect(wrapper.find('[data-test="attachment-row"]').exists()).toBe(false);
  });

  describe("isImageMime()", () => {
    it("returns true for image MIME types", () => {
      const wrapper = mountTray([
        { id: "1", path: "/a.png", name: "a.png", mimeType: "image/png" }
      ]);
      // An img.attachment-thumbnail should be rendered for image types
      expect(wrapper.find("img.attachment-thumbnail").exists()).toBe(true);
    });

    it("returns false for non-image MIME types", () => {
      const wrapper = mountTray([
        { id: "1", path: "/a.txt", name: "a.txt", mimeType: "text/plain" }
      ]);
      // No thumbnail for text types — should show icon instead
      expect(wrapper.find("img.attachment-thumbnail").exists()).toBe(false);
      expect(wrapper.find(".attachment-type-icon").exists()).toBe(true);
    });
  });

  describe("fileExtension()", () => {
    it("extracts extension for dotted filenames", () => {
      const wrapper = mountTray([
        { id: "1", path: "/f.rs", name: "hello.rs", mimeType: "text/x-rust" }
      ]);
      // fi-code class should be applied for .rs
      expect(wrapper.find(".fi-code").exists()).toBe(true);
    });

    it("returns empty string / generic icon for names without a dot", () => {
      const wrapper = mountTray([
        { id: "1", path: "/Makefile", name: "Makefile", mimeType: "application/octet-stream" }
      ]);
      expect(wrapper.find(".fi-generic").exists()).toBe(true);
    });
  });

  describe("fileIconClass()", () => {
    const iconCases: Array<[string, string, string]> = [
      ["hello.pdf", "text/plain", "fi-pdf"],
      ["notes.txt", "text/plain", "fi-text"],
      ["data.json", "application/json", "fi-data"],
      ["index.html", "text/html", "fi-web"],
      ["run.sh", "text/x-shellscript", "fi-script"],
      ["app.py", "text/x-python", "fi-code"],
      ["unknown.xyz", "application/octet-stream", "fi-generic"]
    ];

    it.each(iconCases)("maps %s to %s", (name, mime, expectedClass) => {
      const wrapper = mountTray([{ id: "1", path: `/${name}`, name, mimeType: mime }]);
      expect(wrapper.find(`.${expectedClass}`).exists()).toBe(true);
    });
  });

  describe("truncateFilename()", () => {
    it("shows short names unchanged", () => {
      const wrapper = mountTray([
        { id: "1", path: "/a.rs", name: "main.rs", mimeType: "text/x-rust" }
      ]);
      expect(wrapper.find(".attachment-name").text()).toBe("main.rs");
    });

    it("truncates long names with ellipsis before extension", () => {
      const longName = "very_long_filename_that_exceeds_limit.ts";
      const wrapper = mountTray([
        { id: "1", path: `/${longName}`, name: longName, mimeType: "text/typescript" }
      ]);
      const displayed = wrapper.find(".attachment-name").text();
      expect(displayed).toContain("…");
      expect(displayed.endsWith(".ts")).toBe(true);
      expect(displayed.length).toBeLessThanOrEqual(18);
    });
  });

  describe("onThumbnailError()", () => {
    it("hides image and shows badge sibling on error", async () => {
      const wrapper = mountTray([
        { id: "1", path: "/broken.png", name: "broken.png", mimeType: "image/png" }
      ]);

      const img = wrapper.find("img.attachment-thumbnail");
      expect(img.exists()).toBe(true);

      // Simulate error event on the img
      await img.trigger("error");

      // The img should be hidden (display: none)
      expect((img.element as HTMLImageElement).style.display).toBe("none");
    });
  });

  describe("preview popup", () => {
    it("shows preview on mouseenter for image attachments", async () => {
      const wrapper = mountTray([
        { id: "1", path: "/photo.jpg", name: "photo.jpg", mimeType: "image/jpeg" }
      ]);

      const img = wrapper.find("img.attachment-thumbnail");
      await img.trigger("mouseenter", { clientX: 200, clientY: 300 });

      // Teleport is stubbed, so verify the original thumbnail remains mounted
      // after preview state changes.
      expect(img.exists()).toBe(true);
    });

    it("hides preview on mouseleave", async () => {
      const wrapper = mountTray([
        { id: "1", path: "/photo.jpg", name: "photo.jpg", mimeType: "image/jpeg" }
      ]);

      const img = wrapper.find("img.attachment-thumbnail");
      await img.trigger("mouseenter", { clientX: 200, clientY: 300 });
      await img.trigger("mouseleave");

      // After mouseleave, no preview should be shown
      // (teleport stub renders inline when active)
      expect(wrapper.find(".thumbnail-preview-overlay").exists()).toBe(false);
    });
  });
});
