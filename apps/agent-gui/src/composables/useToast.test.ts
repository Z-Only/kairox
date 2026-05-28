import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useToast } from "./useToast";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("useToast", () => {
  it("success() adds a success toast", () => {
    const { success } = useToast();
    const ui = useUiStore();
    success("Saved!");
    expect(ui.toasts).toHaveLength(1);
    expect(ui.toasts[0].type).toBe("success");
    expect(ui.toasts[0].message).toBe("Saved!");
  });

  it("error() adds an error toast with 8000ms default duration", () => {
    const { error } = useToast();
    const ui = useUiStore();
    error("Something broke");
    expect(ui.toasts).toHaveLength(1);
    expect(ui.toasts[0].type).toBe("error");
    expect(ui.toasts[0].message).toBe("Something broke");
    expect(ui.toasts[0].duration).toBe(8000);
  });

  it("info() adds an info toast", () => {
    const { info } = useToast();
    const ui = useUiStore();
    info("FYI");
    expect(ui.toasts).toHaveLength(1);
    expect(ui.toasts[0].type).toBe("info");
    expect(ui.toasts[0].message).toBe("FYI");
  });

  it("warning() adds a warning toast", () => {
    const { warning } = useToast();
    const ui = useUiStore();
    warning("Be careful");
    expect(ui.toasts).toHaveLength(1);
    expect(ui.toasts[0].type).toBe("warning");
    expect(ui.toasts[0].message).toBe("Be careful");
  });

  it("respects custom duration parameter", () => {
    const { success } = useToast();
    const ui = useUiStore();
    success("Quick", 2000);
    expect(ui.toasts[0].duration).toBe(2000);
  });

  it("error() respects custom duration override", () => {
    const { error } = useToast();
    const ui = useUiStore();
    error("Custom", 3000);
    expect(ui.toasts[0].duration).toBe(3000);
  });
});
