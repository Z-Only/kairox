import { describe, it, expect, vi, beforeEach } from "vitest";
import { nextTick } from "vue";
import { useSidebarRename } from "./useSidebarRename";

describe("useSidebarRename", () => {
  let onConfirm: ReturnType<typeof vi.fn>;
  let onStart: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onConfirm = vi.fn();
    onStart = vi.fn();
  });

  it("initialises with null editingId and empty title", () => {
    const ctrl = useSidebarRename({ onConfirm });
    expect(ctrl.editingId.value).toBeNull();
    expect(ctrl.title.value).toBe("");
    expect(ctrl.input.value).toBeNull();
  });

  describe("start", () => {
    it("sets editingId and title", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "My Session");
      expect(ctrl.editingId.value).toBe("ses_1");
      expect(ctrl.title.value).toBe("My Session");
    });

    it("invokes onStart callback when provided", () => {
      const ctrl = useSidebarRename({ onConfirm, onStart });
      ctrl.start("ses_1", "Title");
      expect(onStart).toHaveBeenCalledOnce();
    });

    it("does not throw when onStart is not provided", () => {
      const ctrl = useSidebarRename({ onConfirm });
      expect(() => ctrl.start("ses_1", "Title")).not.toThrow();
    });

    it("schedules focus and select on the input after nextTick", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      const mockInput = { focus: vi.fn(), select: vi.fn() } as unknown as HTMLInputElement;
      ctrl.input.value = mockInput;

      ctrl.start("ses_1", "Title");
      await nextTick();

      expect(mockInput.focus).toHaveBeenCalledOnce();
      expect(mockInput.select).toHaveBeenCalledOnce();
    });

    it("handles null input ref gracefully on nextTick", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "Title");
      await nextTick();
      // Should not throw even though input.value is null
    });
  });

  describe("bindInput", () => {
    it("binds the element when itemId matches editingId", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "Title");
      const el = document.createElement("input");
      ctrl.bindInput(el, "ses_1");
      expect(ctrl.input.value).toBe(el);
    });

    it("does not bind when itemId does not match editingId", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "Title");
      const el = document.createElement("input");
      ctrl.bindInput(el, "ses_other");
      expect(ctrl.input.value).toBeNull();
    });

    it("sets input to null when element is null and id matches", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "Title");
      ctrl.input.value = document.createElement("input");
      ctrl.bindInput(null, "ses_1");
      expect(ctrl.input.value).toBeNull();
    });
  });

  describe("confirm", () => {
    it("calls onConfirm with trimmed title and resets editingId", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "  Trimmed Title  ");
      await ctrl.confirm();
      expect(onConfirm).toHaveBeenCalledWith("ses_1", "Trimmed Title");
      expect(ctrl.editingId.value).toBeNull();
    });

    it("does not call onConfirm when title is empty", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "");
      await ctrl.confirm();
      expect(onConfirm).not.toHaveBeenCalled();
      expect(ctrl.editingId.value).toBeNull();
    });

    it("does not call onConfirm when title is whitespace only", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "   ");
      await ctrl.confirm();
      expect(onConfirm).not.toHaveBeenCalled();
    });

    it("does not call onConfirm when editingId is null", async () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.title.value = "something";
      await ctrl.confirm();
      expect(onConfirm).not.toHaveBeenCalled();
    });

    it("awaits async onConfirm", async () => {
      let resolved = false;
      const asyncConfirm = vi.fn(async () => {
        await new Promise((r) => setTimeout(r, 0));
        resolved = true;
      });
      const ctrl = useSidebarRename({ onConfirm: asyncConfirm });
      ctrl.start("ses_1", "Title");
      await ctrl.confirm();
      expect(resolved).toBe(true);
      expect(ctrl.editingId.value).toBeNull();
    });
  });

  describe("cancel", () => {
    it("resets editingId to null", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.start("ses_1", "Title");
      expect(ctrl.editingId.value).toBe("ses_1");
      ctrl.cancel();
      expect(ctrl.editingId.value).toBeNull();
    });

    it("is a no-op when not editing", () => {
      const ctrl = useSidebarRename({ onConfirm });
      ctrl.cancel();
      expect(ctrl.editingId.value).toBeNull();
    });
  });

  describe("return shape", () => {
    it("exposes all documented controller properties", () => {
      const ctrl = useSidebarRename({ onConfirm });
      expect(ctrl).toHaveProperty("editingId");
      expect(ctrl).toHaveProperty("title");
      expect(ctrl).toHaveProperty("input");
      expect(ctrl).toHaveProperty("start");
      expect(ctrl).toHaveProperty("bindInput");
      expect(ctrl).toHaveProperty("confirm");
      expect(ctrl).toHaveProperty("cancel");
    });
  });
});
