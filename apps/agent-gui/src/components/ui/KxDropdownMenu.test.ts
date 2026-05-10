import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxDropdownMenu from "./KxDropdownMenu.vue";

describe("KxDropdownMenu", () => {
  it("renders trigger, content, and menu item slots", () => {
    const hostElement = document.createElement("div");
    document.body.appendChild(hostElement);

    const wrapper = mount(KxDropdownMenu, {
      attachTo: hostElement,
      props: {
        open: true,
        contentDataTest: "menu-content"
      },
      slots: {
        trigger: '<button data-test="menu-trigger" type="button">Actions</button>',
        item: '<button data-test="rename-item" type="button">Rename</button>'
      },
      global: {
        stubs: {
          Teleport: true
        }
      }
    });

    try {
      expect(wrapper.find('[data-test="menu-trigger"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="menu-content"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="menu-content"]').classes()).toContain("kx-dropdown-content");
      const menuItem = wrapper.find('[data-test="rename-item"]');

      expect(menuItem.exists()).toBe(true);
      expect(menuItem.classes()).toContain("kx-dropdown-item");
      expect(menuItem.text()).toBe("Rename");
    } finally {
      wrapper.unmount();
      hostElement.remove();
    }
  });

  it("does not treat default slot buttons as menu items", () => {
    const hostElement = document.createElement("div");
    document.body.appendChild(hostElement);

    const wrapper = mount(KxDropdownMenu, {
      attachTo: hostElement,
      props: {
        open: true,
        contentDataTest: "menu-content"
      },
      slots: {
        trigger: '<button data-test="menu-trigger" type="button">Actions</button>',
        default:
          '<button class="kx-dropdown-item" data-test="raw-default-item" type="button">Rename</button>'
      },
      global: {
        stubs: {
          Teleport: true
        }
      }
    });

    try {
      expect(wrapper.find('[data-test="menu-content"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="raw-default-item"]').exists()).toBe(false);
    } finally {
      wrapper.unmount();
      hostElement.remove();
    }
  });
});
