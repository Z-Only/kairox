import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import { confirmDialogKey, useConfirm, type ConfirmAPI } from "./useConfirm";

describe("useConfirm", () => {
  it("throws when no ConfirmDialog provider is present", () => {
    const Wrapper = defineComponent({
      setup() {
        expect(() => useConfirm()).toThrowError(
          "useConfirm() requires <ConfirmDialog /> to be mounted in a parent component"
        );
        return () => null;
      }
    });

    mount(Wrapper);
  });

  it("returns the injected ConfirmAPI when provider exists", () => {
    const mockConfirm = vi.fn().mockResolvedValue(true);
    const api: ConfirmAPI = { confirm: mockConfirm };

    let result: ConfirmAPI | undefined;
    const Child = defineComponent({
      setup() {
        result = useConfirm();
        return () => null;
      }
    });

    mount(Child, {
      global: {
        provide: { [confirmDialogKey as symbol]: api }
      }
    });

    expect(result).toBeDefined();
    expect(result!.confirm).toBe(mockConfirm);
  });

  it("confirm() delegates to the provided implementation", async () => {
    const mockConfirm = vi.fn().mockResolvedValue(false);
    const api: ConfirmAPI = { confirm: mockConfirm };

    let confirmFn: ConfirmAPI["confirm"] | undefined;
    const Child = defineComponent({
      setup() {
        const { confirm } = useConfirm();
        confirmFn = confirm;
        return () => null;
      }
    });

    mount(Child, {
      global: {
        provide: { [confirmDialogKey as symbol]: api }
      }
    });

    const result = await confirmFn!({ message: "Are you sure?" });
    expect(mockConfirm).toHaveBeenCalledWith({ message: "Are you sure?" });
    expect(result).toBe(false);
  });
});
