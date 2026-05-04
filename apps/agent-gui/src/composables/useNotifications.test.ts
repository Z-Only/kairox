import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  notifications,
  addNotification,
  dismissNotification
} from "./useNotifications";

describe("useNotifications", () => {
  beforeEach(() => {
    notifications.splice(0, notifications.length);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("addNotification pushes to array", () => {
    addNotification("info", "hello world");
    expect(notifications).toHaveLength(1);
    expect(notifications[0].type).toBe("info");
    expect(notifications[0].message).toBe("hello world");
  });

  it("addNotification auto-increments IDs", () => {
    addNotification("info", "first");
    addNotification("error", "second");
    expect(notifications).toHaveLength(2);
    expect(notifications[0].id).not.toBe(notifications[1].id);
  });

  it("dismissNotification removes by id", () => {
    addNotification("info", "removeme");
    const id = notifications[0].id;
    dismissNotification(id);
    expect(notifications).toHaveLength(0);
  });

  it("dismissNotification no crash on missing id", () => {
    addNotification("info", "stay");
    dismissNotification("nonexistent-id");
    expect(notifications).toHaveLength(1);
  });

  it("auto-dismiss after 8 seconds", () => {
    vi.useFakeTimers();
    addNotification("warning", "timed");
    expect(notifications).toHaveLength(1);
    vi.advanceTimersByTime(8000);
    expect(notifications).toHaveLength(0);
  });

  it("type differentiation", () => {
    addNotification("error", "err");
    addNotification("warning", "warn");
    addNotification("info", "inf");
    expect(notifications).toHaveLength(3);
    expect(notifications[0].type).toBe("error");
    expect(notifications[1].type).toBe("warning");
    expect(notifications[2].type).toBe("info");
  });
});
