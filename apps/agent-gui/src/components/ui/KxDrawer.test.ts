import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import KxDrawer from "./KxDrawer.vue";
import kxDrawerSource from "./KxDrawer.vue?raw";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("KxDrawer", () => {
  it("renders a right-side drawer with stable panel and body selectors", async () => {
    const wrapper = mount(KxDrawer, {
      props: {
        title: "Catalog detail",
        panelDataTest: "catalog-panel",
        bodyDataTest: "catalog-detail",
        closeLabel: "Close panel"
      },
      slots: {
        default: "<p>Drawer body</p>",
        footer: "<button>Install</button>"
      },
      attachTo: document.body
    });

    expect(document.body.querySelector('[data-test="catalog-panel"]')?.textContent).toContain(
      "Catalog detail"
    );
    expect(document.body.querySelector('[data-test="catalog-detail"]')?.textContent).toContain(
      "Drawer body"
    );

    document.body.querySelector<HTMLButtonElement>(".kx-drawer__close")?.click();
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted("close")).toHaveLength(1);
    wrapper.unmount();
  });

  it("emits close when the backdrop is clicked", async () => {
    const wrapper = mount(KxDrawer, {
      props: { title: "Skill detail" },
      slots: { default: "Detail" },
      attachTo: document.body
    });

    document.body.querySelector<HTMLElement>(".kx-drawer__overlay")?.click();
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted("close")).toHaveLength(1);
    wrapper.unmount();
  });

  it("uses KxIconButton for the close control instead of global btn chrome", () => {
    expect(kxDrawerSource).toContain("KxIconButton");
    expect(kxDrawerSource).not.toContain('class="btn kx-drawer__close drawer-close-btn"');
  });
});
