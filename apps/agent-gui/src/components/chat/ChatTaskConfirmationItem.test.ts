import { describe, it, expect, beforeEach, vi } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatTaskConfirmationItem from "@/components/chat/ChatTaskConfirmationItem.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

function mountItem(props: Record<string, unknown>) {
  return mountWithPlugins(ChatTaskConfirmationItem, {
    props,
    attachTo: document.body
  });
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("ChatTaskConfirmationItem", () => {
  const baseProps = {
    id: "confirm-1",
    prompt: "Choose implementation path",
    options: [
      { id: "small", label: "Small fix", description: "Touch one module" },
      { id: "broad", label: "Broad pass", description: "Update related surfaces" }
    ],
    allowMultiple: true,
    allowCustom: true
  };

  it("renders prompt, options, and a custom response field", () => {
    const wrapper = mountItem(baseProps);

    expect(wrapper.find('[data-test="chat-task-confirmation-item"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("Choose implementation path");
    expect(wrapper.text()).toContain("Small fix");
    expect(wrapper.text()).toContain("Touch one module");
    expect(wrapper.find('[data-test="task-confirmation-custom"]').exists()).toBe(true);
  });

  it("submits selected options and custom response", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mountItem(baseProps);

    await wrapper.find('[data-test="task-confirmation-option-small"]').setValue(true);
    await wrapper.find('[data-test="task-confirmation-custom"]').setValue("Keep public API stable");
    await wrapper.find('[data-test="task-confirmation-submit"]').trigger("click");

    expect(mockedInvoke).toHaveBeenCalledWith("resolve_task_confirmation", {
      decision: {
        request_id: "confirm-1",
        selected_option_ids: ["small"],
        custom_response: "Keep public API stable"
      }
    });
  });

  it("uses radio behavior when allowMultiple is false", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mountItem({ ...baseProps, allowMultiple: false, allowCustom: false });

    await wrapper.find('[data-test="task-confirmation-option-small"]').setValue(true);
    await wrapper.find('[data-test="task-confirmation-option-broad"]').setValue(true);
    await wrapper.find('[data-test="task-confirmation-submit"]').trigger("click");

    expect(mockedInvoke).toHaveBeenCalledWith("resolve_task_confirmation", {
      decision: {
        request_id: "confirm-1",
        selected_option_ids: ["broad"],
        custom_response: null
      }
    });
  });

  it("keeps submit disabled until an option or custom response is present", async () => {
    const wrapper = mountItem({ ...baseProps, allowMultiple: true, allowCustom: true });
    const submit = wrapper.find<HTMLButtonElement>('[data-test="task-confirmation-submit"]');

    expect(submit.element.disabled).toBe(true);
    await wrapper.find('[data-test="task-confirmation-custom"]').setValue("Different option");
    expect(submit.element.disabled).toBe(false);
  });
});
