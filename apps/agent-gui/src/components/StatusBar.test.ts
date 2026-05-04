import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import StatusBar from "./StatusBar.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

describe("StatusBar", () => {
  it("calls get_permission_mode on mount", () => {
    mockedInvoke.mockResolvedValueOnce("Suggest");
    mount(StatusBar);
    expect(mockedInvoke).toHaveBeenCalledWith("get_permission_mode");
  });

  it("displays the permission mode in lowercase", async () => {
    mockedInvoke.mockResolvedValueOnce("Suggest");
    const wrapper = mount(StatusBar);
    await vi.waitFor(() => {
      expect(wrapper.text()).toContain("suggest");
    });
  });
});
