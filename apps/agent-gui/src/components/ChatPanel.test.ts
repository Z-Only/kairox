import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import ChatPanel from "./ChatPanel.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { sessionState, resetProjection } from "../stores/session";

beforeEach(() => {
  resetProjection();
  sessionState.currentSessionId = "ses_1";
  sessionState.currentProfile = "fast";
  sessionState.isStreaming = false;
  vi.clearAllMocks();
});

describe("ChatPanel", () => {
  it("renders user messages from projection", () => {
    sessionState.projection.messages = [{ role: "user", content: "Hello" }];
    const wrapper = mount(ChatPanel);
    expect(wrapper.text()).toContain("Hello");
    expect(wrapper.text()).toContain("You");
  });

  it("renders assistant messages", () => {
    sessionState.projection.messages = [
      { role: "assistant", content: "Hi there!" }
    ];
    const wrapper = mount(ChatPanel);
    expect(wrapper.text()).toContain("Hi there!");
    expect(wrapper.text()).toContain("Agent");
  });

  it("shows streaming text with cursor when isStreaming", () => {
    sessionState.projection.token_stream = "Loading...";
    sessionState.isStreaming = true;
    const wrapper = mount(ChatPanel);
    expect(wrapper.text()).toContain("Loading...");
    expect(wrapper.find(".cursor").exists()).toBe(true);
  });

  it("shows cancelled marker", () => {
    sessionState.projection.cancelled = true;
    const wrapper = mount(ChatPanel);
    expect(wrapper.text()).toContain("[cancelled]");
  });

  it("shows Cancel button during streaming", () => {
    sessionState.isStreaming = true;
    const wrapper = mount(ChatPanel);
    expect(wrapper.find(".cancel-button").exists()).toBe(true);
    expect(wrapper.find(".send-button").exists()).toBe(false);
  });

  it("shows Send button when not streaming", () => {
    sessionState.isStreaming = false;
    const wrapper = mount(ChatPanel);
    expect(wrapper.find(".send-button").exists()).toBe(true);
    expect(wrapper.find(".cancel-button").exists()).toBe(false);
  });

  it("disables textarea when isStreaming", () => {
    sessionState.isStreaming = true;
    const wrapper = mount(ChatPanel);
    expect(wrapper.find(".message-input").attributes("disabled")).toBeDefined();
  });

  it("invokes cancel_session on Cancel click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    sessionState.isStreaming = true;
    const wrapper = mount(ChatPanel);
    await wrapper.find(".cancel-button").trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("cancel_session");
  });
});
