import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { mountWithPlugins } from "@/test-utils/mount";
import en from "@/locales/en.json";
import zhCN from "@/locales/zh-CN.json";
import CommandPalette from "./CommandPalette.vue";
import commandPaletteSource from "./CommandPalette.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

// ---- Mocks ----

const paletteStoreMocks = vi.hoisted(() => ({
  resetProjection: vi.fn(),
  skills: [] as Array<{ id: string; name: string }>,
  profileInfos: [] as Array<{
    alias: string;
    provider: string;
    model_id: string;
    provider_display?: string;
    model_display?: string;
  }>
}));

// Session store: a valid session so session-active commands appear
vi.mock("@/stores/session", () => ({
  useSessionStore: () => ({
    currentSessionId: "ses_1",
    resetProjection: paletteStoreMocks.resetProjection,
    profileInfos: paletteStoreMocks.profileInfos
  }),
  formatProfileDisplay: (p: { alias: string; provider_display?: string; model_display?: string }) =>
    [p.alias, p.provider_display, p.model_display].filter(Boolean).join(" · ")
}));

// Skills store: empty so only built-in commands show in initial render
vi.mock("@/stores/skills", () => ({
  useSkillsStore: () => ({
    skills: paletteStoreMocks.skills
  })
}));

// Tauri invoke (compact command handler uses it)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

describe("CommandPalette", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    paletteStoreMocks.resetProjection.mockClear();
    paletteStoreMocks.skills.splice(0);
    paletteStoreMocks.profileInfos.splice(0);
  });

  // ---- Rendering ----

  it("renders when visible with items", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const palette = wrapper.find('[data-test="command-palette"]');
    expect(palette.exists()).toBe(true);
    expect(palette.classes()).toContain("kx-popover-content");
    expect(palette.classes()).toContain("kx-popover-content--palette");
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
    expect(header.classes()).toContain("kx-popover-panel__header");
    expect(header.text()).toBe("Commands, Models & Skills");
  });

  it("renders command palette chrome and command descriptions from the active locale", () => {
    const i18n = createI18n({
      legacy: false,
      locale: "zh-CN",
      fallbackLocale: "en",
      messages: { en, "zh-CN": zhCN }
    });
    const wrapper = mount(CommandPalette, {
      props: { visible: true, filterText: "" },
      global: { plugins: [i18n] }
    });

    expect(wrapper.find(".command-palette__header").text()).toBe("命令、模型与技能");
    expect(wrapper.find('[data-test="palette-item-clear"] .command-palette__desc').text()).toBe(
      "清空当前对话"
    );
    expect(wrapper.text()).not.toContain("Clear the current conversation");
  });

  it("does not keep command palette chrome copy inline in the component source", () => {
    expectSourceMigration(commandPaletteSource, {
      forbidden: ["Commands, Models & Skills", "Run skill", "Switch model"]
    });
  });

  it("renders builtin command items with BEM class", () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "" } },
      reusePinia: true
    });
    const items = wrapper.findAll(".command-palette__item");
    expect(items.map((item) => item.find(".command-palette__label").text())).toEqual([
      "/clear",
      "/compact",
      "/model",
      "/goal",
      "/help",
      "/instructions",
      "/hooks",
      "/skills",
      "/agents",
      "/plugins",
      "/mcp",
      "/models"
    ]);
    expect(items[0].classes()).toContain("kx-popover-option");
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

  it("keeps the palette visible with a shared popover empty state when nothing matches", async () => {
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "definitely-no-command-match" } },
      reusePinia: true
    });
    await wrapper.vm.$nextTick();

    const palette = wrapper.find('[data-test="command-palette"]');
    const empty = wrapper.find('[data-test="command-palette-empty"]');
    expect(palette.exists()).toBe(true);
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toBe("No commands, models, or skills match");
    expect(empty.classes()).toContain("kx-empty-state");
    expect(empty.classes()).toContain("kx-empty-state--popover");
    expect(empty.classes()).toContain("kx-popover-empty");
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
    expect(items[1].classes()).toContain("kx-popover-option--selected");
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

  it("emits select-skill and closes when clicking a discovered skill item", async () => {
    paletteStoreMocks.skills.push({
      id: "workspace-review",
      name: "Workspace Review"
    });
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "workspace" } },
      reusePinia: true
    });

    const item = wrapper.find('[data-test="palette-item-workspace-review"]');
    expect(item.exists()).toBe(true);
    expect(item.find(".command-palette__label").text()).toBe("/skills Workspace Review");

    await item.trigger("click");

    expect(wrapper.emitted("select-skill")).toEqual([["workspace-review"]]);
    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("emits select-model-profile and closes when clicking a model profile item", async () => {
    paletteStoreMocks.profileInfos.push({
      alias: "daily-driver",
      provider: "openai",
      model_id: "gpt-4.1",
      provider_display: "OpenAI",
      model_display: "GPT-4.1"
    });
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "daily" } },
      reusePinia: true
    });

    const item = wrapper.find('[data-test="palette-item-daily-driver"]');
    expect(item.exists()).toBe(true);
    expect(item.find(".command-palette__label").text()).toBe("daily-driver · OpenAI · GPT-4.1");

    await item.trigger("click");

    expect(wrapper.emitted("select-model-profile")).toEqual([["daily-driver"]]);
    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("emits select-model-profile and closes when pressing Enter on a model profile item", async () => {
    paletteStoreMocks.profileInfos.push({
      alias: "fast-local",
      provider: "ollama",
      model_id: "qwen3",
      provider_display: "Ollama",
      model_display: "Qwen3"
    });
    const { wrapper } = mountWithPlugins(CommandPalette, {
      mount: { props: { visible: true, filterText: "fast-local" } },
      reusePinia: true
    });

    const items = wrapper.findAll(".command-palette__item");
    expect(items).toHaveLength(1);

    await wrapper.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("select-model-profile")).toEqual([["fast-local"]]);
    expect(wrapper.emitted("close")).toHaveLength(1);
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
    expect(wrapper.find('[data-test="palette-item-goal"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="palette-item-help"]').exists()).toBe(true);
  });
});
