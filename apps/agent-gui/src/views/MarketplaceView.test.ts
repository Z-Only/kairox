import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { enableAutoUnmount, flushPromises } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import MarketplaceView from "./MarketplaceView.vue";

// ---------------------------------------------------------------------------
// Test environment
// ---------------------------------------------------------------------------

enableAutoUnmount(afterEach);

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("MarketplaceView", () => {
  function mountView() {
    const opts: MountWithPluginsOptions<typeof MarketplaceView> = {
      reusePinia: true,
      mount: {
        global: {
          stubs: {
            MarketplacePane: { template: '<div data-test="marketplace-pane-stub" />' }
          }
        }
      }
    };
    return mountWithPlugins(MarketplaceView, opts).wrapper;
  }

  it("renders the marketplace container with data-test attribute", async () => {
    const wrapper = mountView();
    await flushPromises();

    expect(wrapper.find('[data-test="view-marketplace"]').exists()).toBe(true);
  });

  it("renders the .marketplace.card wrapper element", async () => {
    const wrapper = mountView();
    await flushPromises();

    const el = wrapper.find(".marketplace.card");
    expect(el.exists()).toBe(true);
  });

  it("renders the MarketplacePane child component", async () => {
    const wrapper = mountView();
    await flushPromises();

    expect(wrapper.find('[data-test="marketplace-pane-stub"]').exists()).toBe(true);
  });
});
