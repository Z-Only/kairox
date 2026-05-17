import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import CommandPalette from "./CommandPalette.vue";

// ---- Mocks ----

// Session store: a valid session so session-active commands appear
vi.mock("@/stores/session", () => ({
  useSessionStore: () => ({
    currentSessionId: "ses_1",
    resetProjection: vi.fn(),
    profileInfos: []
  }),
  formatProfileDisplay: (p: { alias: string }) => p.alias
}));

// Skills store: empty so only built-in commands show in initial render
vi.mock("@/stores/skills", () => ({
  useSkillsStore: () => ({
    activeSkills: []
  })
}));

// Tauri invoke (compact command handler uses it)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

describe("CommandPalette", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  // ---- Rendering ----

  it("renders when visible with items", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    expect(wrapper.find('[data-test="command-palette"]').exists()).toBe(true);
  });

  it("hides when not visible", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: false, filterText: "" } },
      reusePinia: true
    });
    expect(wrapper.find('[data-test="command-palette"]').exists()).toBe(false);
  });

  it("renders a header label", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const header = wrapper.find(".command-palette__header");
    expect(header.exists()).toBe(true);
    expect(header.text()).toBe("Commands, Models & Skills");
  });

  it("renders builtin command items with BEM class", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const items = wrapper.findAll(".command-palette__item");
    // 4 builtins: clear, compact, model, help
    expect(items.length).toBe(4);
  });

  // ---- Filtering ----

  it("filters items when filterText prop changes", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    // Watcher fires on prop CHANGE, not initial mount
    await wrapper.setProps({ filterText: "clear" });
    await wrapper.vm.$nextTick();
    const items = wrapper.findAll(".command-palette__item");
    expect(items.length).toBe(1);
    // The item label should contain "/clear"
    expect(items[0].find(".command-palette__label").text()).toBe("/clear");
  });

  // ---- Keyboard events ----

  it("emits close on Escape keydown", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    await wrapper.trigger("keydown", { key: "Escape" });
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("moves selection with ArrowDown", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    await wrapper.trigger("keydown", { key: "ArrowDown" });
    await wrapper.vm.$nextTick();
    // First item should no longer be selected, second should be
    const items = wrapper.findAll(".command-palette__item");
    expect(items[0].classes()).not.toContain("command-palette__item--selected");
    expect(items[1].classes()).toContain("command-palette__item--selected");
  });

  it("moves selection with ArrowUp from second to first", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    // Move to index 1 first
    await wrapper.trigger("keydown", { key: "ArrowDown" });
    await wrapper.vm.$nextTick();
    // Now move back up
    await wrapper.trigger("keydown", { key: "ArrowUp" });
    await wrapper.vm.$nextTick();
    const items = wrapper.findAll(".command-palette__item");
    expect(items[0].classes()).toContain("command-palette__item--selected");
    expect(items[1].classes()).not.toContain("command-palette__item--selected");
  });

  it("does not move selection below last item", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const count = wrapper.findAll(".command-palette__item").length;
    // Move past the end
    for (let i = 0; i < count + 2; i++) {
      await wrapper.trigger("keydown", { key: "ArrowDown" });
      await wrapper.vm.$nextTick();
    }
    const items = wrapper.findAll(".command-palette__item");
    // Last item should be selected, not overflow
    expect(items[count - 1].classes()).toContain("command-palette__item--selected");
  });

  // ---- Mouse events ----

  it("emits select-command when clicking an insertText command", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    // "/model" command has insertText (no handler), so clicking emits select-command
    const item = wrapper.find('[data-test="palette-item-model"]');
    expect(item.exists()).toBe(true);
    await item.trigger("click");
    expect(wrapper.emitted("select-command")).toBeTruthy();
    // The emitted payload should be a command object
    const emitted = wrapper.emitted("select-command")!;
    expect(emitted[0][0]).toHaveProperty("id", "model");
  });

  it("updates selection on mouseenter", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const items = wrapper.findAll(".command-palette__item");
    // Hover over the third item (index 2)
    await items[2].trigger("mouseenter");
    await wrapper.vm.$nextTick();
    expect(items[2].classes()).toContain("command-palette__item--selected");
    expect(items[0].classes()).not.toContain("command-palette__item--selected");
  });

  it("emits close when clicking a handler command (clear)", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    // "clear" command has handler; clicking it calls handler then emits close
    const item = wrapper.find('[data-test="palette-item-clear"]');
    expect(item.exists()).toBe(true);
    await item.trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  // ---- data-test anchors ----

  it("provides data-test anchor on palette root", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    expect(wrapper.find('[data-test="command-palette"]').exists()).toBe(true);
  });

  it("provides data-test anchors on each item", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    expect(wrapper.find('[data-test="palette-item-clear"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="palette-item-compact"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="palette-item-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="palette-item-help"]').exists()).toBe(true);
  });
});
