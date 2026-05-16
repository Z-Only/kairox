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
});
