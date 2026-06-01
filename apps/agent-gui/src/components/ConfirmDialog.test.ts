import { defineComponent, h, inject, nextTick } from "vue";
import { describe, it, expect } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

import ConfirmDialog from "./ConfirmDialog.vue";
import confirmDialogSource from "./ConfirmDialog.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { confirmDialogKey, type ConfirmAPI } from "@/composables/useConfirm";

/**
 * Helper: mount ConfirmDialog with a child component that injects the
 * confirm API so tests can drive the dialog programmatically.
 */
function mountWithConsumer() {
  let api: ConfirmAPI | undefined;
  const Consumer = defineComponent({
    setup() {
      api = inject(confirmDialogKey);
      return () => h("span", { id: "consumer" }, "child");
    }
  });

  const wrapper = mount(ConfirmDialog, {
    slots: { default: () => h(Consumer) },
    global: {
      stubs: {
        KxModal: {
          template: `<div v-if="open" data-test="confirm-dialog"><slot /><slot name="footer" /></div>`,
          props: ["open", "title", "closeLabel", "width"],
          emits: ["close"]
        },
        KxButton: {
          // Don't declare props so data-test, variant, etc. stay in $attrs
          // and get forwarded to the root <button> via inheritAttrs (default true).
          template: `<button><slot /></button>`
        }
      }
    }
  });

  return { wrapper, getApi: () => api! };
}

describe("ConfirmDialog", () => {
  it("uses KxButton for confirm actions instead of global btn variants", () => {
    expectSourceMigration(confirmDialogSource, {
      required: ["KxButton"],
      forbidden: ['class="btn"', "btn-danger", "btn-primary"]
    });
  });

  it("provides confirm API to children via confirmDialogKey", () => {
    const { getApi } = mountWithConsumer();
    expect(getApi()).toBeDefined();
    expect(typeof getApi().confirm).toBe("function");
  });

  it("confirm() opens dialog and resolves true when confirm is clicked", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    // Dialog should be closed initially
    expect(wrapper.find('[data-test="confirm-dialog"]').exists()).toBe(false);

    // Call confirm — don't await yet
    const promise = getApi().confirm({ message: "Delete this?" });
    await nextTick();

    // Dialog should now be visible
    expect(wrapper.find('[data-test="confirm-dialog"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("Delete this?");

    // Click confirm button
    await wrapper.find('[data-test="confirm-ok"]').trigger("click");
    const result = await promise;
    expect(result).toBe(true);

    // Dialog closed
    await nextTick();
    expect(wrapper.find('[data-test="confirm-dialog"]').exists()).toBe(false);
  });

  it("handleCancel resolves false and closes the dialog", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    const promise = getApi().confirm({ message: "Are you sure?" });
    await nextTick();

    expect(wrapper.find('[data-test="confirm-dialog"]').exists()).toBe(true);

    // Click cancel button
    await wrapper.find('[data-test="confirm-cancel"]').trigger("click");
    const result = await promise;
    expect(result).toBe(false);

    await nextTick();
    expect(wrapper.find('[data-test="confirm-dialog"]').exists()).toBe(false);
  });

  it("fills in default options when optional fields are omitted", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    getApi().confirm({ message: "Simple message" });
    await nextTick();

    // Check defaults: title="", confirmText="Confirm", cancelText="Cancel"
    const text = wrapper.text();
    expect(text).toContain("Simple message");
    expect(text).toContain("Confirm");
    expect(text).toContain("Cancel");
  });

  it("applies custom title, confirmText, and cancelText", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    getApi().confirm({
      message: "Do it?",
      title: "My Title",
      confirmText: "Yes!",
      cancelText: "Nope"
    });
    await nextTick();

    const text = wrapper.text();
    expect(text).toContain("Yes!");
    expect(text).toContain("Nope");
  });

  it("sets variant='danger' on confirm button when type is 'error'", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    getApi().confirm({ message: "Danger!", type: "error" });
    await nextTick();

    const confirmBtn = wrapper.find('[data-test="confirm-ok"]');
    expect(confirmBtn.attributes("variant")).toBe("danger");
  });

  it("sets variant='primary' on confirm button when type is 'info' (default)", async () => {
    const { wrapper, getApi } = mountWithConsumer();

    getApi().confirm({ message: "Info" });
    await nextTick();

    const confirmBtn = wrapper.find('[data-test="confirm-ok"]');
    expect(confirmBtn.attributes("variant")).toBe("primary");
  });
});
