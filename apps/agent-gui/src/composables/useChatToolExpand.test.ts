import { describe, it, expect, beforeEach } from "vitest";
import { ref } from "vue";
import { useChatToolExpand } from "./useChatToolExpand";

beforeEach(() => {
  localStorage.clear();
});

describe("useChatToolExpand", () => {
  it("defaults to false when no value is stored", () => {
    const sessionId = ref<string | null>("ses_default");
    const { isExpanded } = useChatToolExpand(sessionId, "tc_1");
    expect(isExpanded.value).toBe(false);
  });

  it("reads a previously stored `true` value on construction", () => {
    localStorage.setItem("kairox.chatToolExpand.ses_a.tc_1", "true");
    const sessionId = ref<string | null>("ses_a");
    const { isExpanded } = useChatToolExpand(sessionId, "tc_1");
    expect(isExpanded.value).toBe(true);
  });

  it("falls back to default on corrupted JSON", () => {
    localStorage.setItem("kairox.chatToolExpand.ses_a.tc_1", "{not json");
    const sessionId = ref<string | null>("ses_a");
    const { isExpanded } = useChatToolExpand(sessionId, "tc_1");
    expect(isExpanded.value).toBe(false);
  });

  it("falls back to default when stored value is not boolean", () => {
    localStorage.setItem("kairox.chatToolExpand.ses_a.tc_1", '"hi"');
    const sessionId = ref<string | null>("ses_a");
    const { isExpanded } = useChatToolExpand(sessionId, "tc_1");
    expect(isExpanded.value).toBe(false);
  });

  it("toggle() flips state and writes to localStorage", () => {
    const sessionId = ref<string | null>("ses_b");
    const { isExpanded, toggle } = useChatToolExpand(sessionId, "tc_2");
    expect(isExpanded.value).toBe(false);

    toggle();
    expect(isExpanded.value).toBe(true);
    expect(localStorage.getItem("kairox.chatToolExpand.ses_b.tc_2")).toBe("true");

    toggle();
    expect(isExpanded.value).toBe(false);
    expect(localStorage.getItem("kairox.chatToolExpand.ses_b.tc_2")).toBe("false");
  });

  it("does not write when sessionId is null", () => {
    const sessionId = ref<string | null>(null);
    const { isExpanded, toggle } = useChatToolExpand(sessionId, "tc_3");
    expect(isExpanded.value).toBe(false);
    toggle();
    expect(isExpanded.value).toBe(true);
    // No persistence key should have been written for a null sessionId
    const keys = Object.keys(localStorage).filter((k) => k.startsWith("kairox.chatToolExpand."));
    expect(keys).toEqual([]);
  });

  it("re-reads the stored value when sessionId changes", async () => {
    localStorage.setItem("kairox.chatToolExpand.ses_x.tc_4", "true");
    localStorage.setItem("kairox.chatToolExpand.ses_y.tc_4", "false");
    const sessionId = ref<string | null>("ses_x");
    const { isExpanded } = useChatToolExpand(sessionId, "tc_4");
    expect(isExpanded.value).toBe(true);

    sessionId.value = "ses_y";
    // Allow Vue's reactivity to flush
    await Promise.resolve();
    expect(isExpanded.value).toBe(false);
  });

  it("tolerates localStorage throwing on read", () => {
    const original = Storage.prototype.getItem;
    Storage.prototype.getItem = () => {
      throw new Error("denied");
    };
    try {
      const sessionId = ref<string | null>("ses_err");
      const { isExpanded } = useChatToolExpand(sessionId, "tc_err");
      expect(isExpanded.value).toBe(false);
    } finally {
      Storage.prototype.getItem = original;
    }
  });

  it("tolerates localStorage throwing on write", () => {
    const original = Storage.prototype.setItem;
    Storage.prototype.setItem = () => {
      throw new Error("quota");
    };
    try {
      const sessionId = ref<string | null>("ses_err");
      const { isExpanded, toggle } = useChatToolExpand(sessionId, "tc_err");
      expect(() => toggle()).not.toThrow();
      // Local state still updates even when persistence fails
      expect(isExpanded.value).toBe(true);
    } finally {
      Storage.prototype.setItem = original;
    }
  });
});
