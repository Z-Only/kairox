import { describe, it, expect, vi, afterEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import ImageLightbox from "./ImageLightbox.vue";

function mountLightbox(props: Partial<InstanceType<typeof ImageLightbox>["$props"]> = {}) {
  return mount(ImageLightbox, {
    props: {
      src: "https://example.com/image.png",
      alt: "Test image",
      ...props
    },
    global: {
      stubs: { Teleport: true }
    }
  });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ImageLightbox", () => {
  it("renders thumbnail with correct src and alt", () => {
    const wrapper = mountLightbox();
    const img = wrapper.find(".lightbox-thumbnail");

    expect(img.exists()).toBe(true);
    expect(img.attributes("src")).toBe("https://example.com/image.png");
    expect(img.attributes("alt")).toBe("Test image");
  });

  it("overlay is hidden by default", () => {
    const wrapper = mountLightbox();

    expect(wrapper.find(".lightbox-overlay").exists()).toBe(false);
  });

  it("opens overlay on thumbnail click", async () => {
    const wrapper = mountLightbox();

    await wrapper.find(".lightbox-thumbnail").trigger("click");

    expect(wrapper.find(".lightbox-overlay").exists()).toBe(true);
    expect(wrapper.find(".lightbox-image").attributes("src")).toBe("https://example.com/image.png");
  });

  it("closes overlay on overlay click", async () => {
    const wrapper = mountLightbox();

    await wrapper.find(".lightbox-thumbnail").trigger("click");
    expect(wrapper.find(".lightbox-overlay").exists()).toBe(true);

    await wrapper.find(".lightbox-overlay").trigger("click");
    expect(wrapper.find(".lightbox-overlay").exists()).toBe(false);
  });

  it("does not close when clicking the enlarged image", async () => {
    const wrapper = mountLightbox();

    await wrapper.find(".lightbox-thumbnail").trigger("click");
    await wrapper.find(".lightbox-image").trigger("click");

    expect(wrapper.find(".lightbox-overlay").exists()).toBe(true);
  });

  it("closes on Escape key", async () => {
    const wrapper = mountLightbox();

    await wrapper.find(".lightbox-thumbnail").trigger("click");
    expect(wrapper.find(".lightbox-overlay").exists()).toBe(true);

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();

    expect(wrapper.find(".lightbox-overlay").exists()).toBe(false);
  });

  it("Escape does nothing when lightbox is closed", async () => {
    const wrapper = mountLightbox();

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();

    expect(wrapper.find(".lightbox-overlay").exists()).toBe(false);
  });

  it("removes keydown listener on unmount", async () => {
    const spy = vi.spyOn(document, "removeEventListener");
    const wrapper = mountLightbox();

    await wrapper.find(".lightbox-thumbnail").trigger("click");
    wrapper.unmount();

    expect(spy).toHaveBeenCalledWith("keydown", expect.any(Function));
  });

  it("renders without alt prop", () => {
    const wrapper = mount(ImageLightbox, {
      props: { src: "https://example.com/no-alt.png" },
      global: { stubs: { Teleport: true } }
    });

    expect(wrapper.find(".lightbox-thumbnail").attributes("alt")).toBeUndefined();
  });
});
