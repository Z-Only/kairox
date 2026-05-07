import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// We mock the *named export* `useMessage` from naive-ui so we can flip its
// behaviour per test. The other exports (NConfigProvider, NMessageProvider,
// theming) used elsewhere in the suite are not pulled into this file, so a
// minimal mock is sufficient.
const useMessageMock = vi.fn();
vi.mock("naive-ui", () => ({
  useMessage: () => useMessageMock()
}));

import { useNotifications } from "./useNotifications";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
  useMessageMock.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useNotifications", () => {
  it("notify() writes a single entry to the ui store (no duplicate visual call)", () => {
    // Provider available — useMessage() resolves to a stub.
    useMessageMock.mockReturnValue({
      success: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      error: vi.fn()
    });

    const { notify } = useNotifications();
    const ui = useUiStore();

    notify("info", "hello");

    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("info");
    expect(ui.notifications[0].message).toBe("hello");
  });

  it("notify() does NOT throw when useMessage() throws (called outside <NMessageProvider>) and still writes to the store", () => {
    // Simulate misuse from outside the provider subtree.
    useMessageMock.mockImplementation(() => {
      throw new Error("useMessage must be called inside an <NMessageProvider>");
    });
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    // Construction must not throw.
    const { notify } = useNotifications();
    const ui = useUiStore();

    // notify() must not throw and must still push to the store so the
    // persistent notification log is preserved.
    expect(() => notify("error", "boom")).not.toThrow();
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("error");
    expect(ui.notifications[0].message).toBe("boom");

    // The provider-missing diagnostic is reported once at composable
    // construction time, plus once per `notify("error", ...)` call (the
    // 7b carry-over #2 degraded-mode trace) — so two console.error calls
    // total here. Exhaustive degraded-mode behaviour is verified in the
    // dedicated "in degraded mode, error-level notify() leaves an
    // additional console trace per event" test below.
    expect(errorSpy).toHaveBeenCalledTimes(2);
    expect(String(errorSpy.mock.calls[0][0])).toContain("useNotifications");
  });

  it("in degraded mode, error-level notify() leaves an additional console trace per event", () => {
    // Carry-over from 7a code-quality review (item #2): when the provider
    // is unavailable, individual `error`-level notifications must still
    // surface to developers via console.error so failures are not silent
    // after the construction-time diagnostic. Non-error levels stay
    // store-only (they're not actionable enough to warrant log noise).
    useMessageMock.mockImplementation(() => {
      throw new Error("useMessage must be called inside an <NMessageProvider>");
    });
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const { notify } = useNotifications();
    const ui = useUiStore();

    // First console.error: construction-time provider-missing diagnostic.
    expect(errorSpy).toHaveBeenCalledTimes(1);

    notify("error", "boom");

    // Second console.error: per-event degraded trace from notify("error").
    expect(errorSpy).toHaveBeenCalledTimes(2);
    expect(String(errorSpy.mock.calls[1][0])).toContain("(degraded)");
    expect(String(errorSpy.mock.calls[1][1])).toBe("boom");
    // Store still receives the entry — persistent log is the source of truth.
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("error");

    notify("info", "fyi");
    // info-level does not add a per-event console trace; only the store
    // receives the entry. The error spy stays at 2.
    expect(errorSpy).toHaveBeenCalledTimes(2);
    expect(ui.notifications).toHaveLength(2);
  });
});
