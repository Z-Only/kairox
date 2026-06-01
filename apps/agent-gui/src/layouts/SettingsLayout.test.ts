import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory } from "vue-router";
import { routes } from "@/router/routes";
import en from "@/locales/en.json";
import SettingsLayout from "./SettingsLayout.vue";
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";

// Stub Tauri APIs that child settings panes pull in transitively.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function makeRouter() {
  return createRouter({ history: createMemoryHistory(), routes });
}

function makeI18n() {
  return createI18n({ legacy: false, locale: "en", messages: { en } });
}

const ConfigSourceBarStub = defineComponent({
  name: "ConfigSourceBar",
  props: {
    initialSource: {
      type: String,
      default: undefined
    },
    initialProjectId: {
      type: String,
      default: undefined
    }
  },
  template:
    '<div data-test="config-source-bar-stub" :data-initial-source="initialSource ?? \'\'" :data-initial-project-id="initialProjectId ?? \'\'" />'
});

beforeEach(() => {
  vi.clearAllMocks();
});

/** Mount SettingsLayout at a given settings sub-route. */
async function mountAt(path = "/settings/general", seedStores?: () => void) {
  const router = makeRouter();
  const pinia = createPinia();
  setActivePinia(pinia);
  seedStores?.();

  await router.push(path);
  await router.isReady();

  const wrapper = mount(SettingsLayout, {
    global: {
      plugins: [router, pinia, makeI18n()],
      stubs: {
        ConfigSourceBar: ConfigSourceBarStub,
        // Stub all child route components to isolate the layout shell
        GeneralSettings: true,
        McpSettingsPane: true,
        SkillSettingsPane: true,
        PluginSettingsPane: true,
        AgentSettingsPane: true,
        ModelSettingsPane: true,
        InstructionsSettingsPane: true,
        HooksSettingsPane: true,
        ArchiveSettingsPane: true
      }
    }
  });

  await flushPromises();
  return { wrapper, router };
}

const ALL_TABS = [
  "general",
  "mcp",
  "skills",
  "plugins",
  "agents",
  "models",
  "instructions",
  "hooks",
  "archive"
] as const;

// Tabs that show the ConfigSourceBar
const SOURCE_BAR_TABS = ["mcp", "skills", "plugins", "agents", "models", "instructions", "hooks"];

function seedOrdinaryConversation(): void {
  const session = useSessionStore();
  session.sessions = [
    {
      id: "ordinary-session-1",
      title: "Ordinary chat",
      profile: "fast",
      project_id: null,
      worktree_path: null,
      branch: null,
      deleted_at: null,
      visibility: null
    }
  ];
  session.currentSessionId = "ordinary-session-1";
}

function seedProjectConversation(): void {
  const session = useSessionStore();
  const project = useProjectStore();
  project.projects = [
    {
      projectId: "project-1",
      displayName: "Project One",
      rootPath: "/tmp/project-one",
      removedAt: null,
      sortOrder: 0,
      expanded: true,
      pathExists: true
    }
  ];
  project.sessionsByProject = new Map([
    [
      "project-1",
      [
        {
          sessionId: "project-session-1",
          title: "Project chat",
          profile: "fast",
          projectId: "project-1",
          worktreePath: "/tmp/project-one",
          branch: "main",
          visibility: "visible",
          deletedAt: null
        }
      ]
    ]
  ]);
  session.currentSessionId = "project-session-1";
}

describe("SettingsLayout", () => {
  // --- Rendering ---

  it("renders the settings container with data-test selector", async () => {
    const { wrapper } = await mountAt();
    expect(wrapper.find('[data-test="view-settings"]').exists()).toBe(true);
  });

  it("renders a page heading", async () => {
    const { wrapper } = await mountAt();
    const heading = wrapper.find("h1");
    expect(heading.exists()).toBe(true);
    expect(heading.text()).toBeTruthy();
  });

  // --- Tab navigation ---

  it("renders all nine settings tabs", async () => {
    const { wrapper } = await mountAt();
    for (const tab of ALL_TABS) {
      expect(wrapper.find(`[data-test="settings-tab-${tab}"]`).exists()).toBe(true);
    }
  });

  it("highlights the active tab matching the current route", async () => {
    const { wrapper } = await mountAt("/settings/mcp");
    const mcpTab = wrapper.get('[data-test="settings-tab-mcp"]');
    expect(mcpTab.attributes("aria-selected")).toBe("true");

    const generalTab = wrapper.get('[data-test="settings-tab-general"]');
    expect(generalTab.attributes("aria-selected")).toBe("false");
  });

  it("defaults to general when on /settings/general", async () => {
    const { wrapper } = await mountAt("/settings/general");
    const generalTab = wrapper.get('[data-test="settings-tab-general"]');
    expect(generalTab.attributes("aria-selected")).toBe("true");
  });

  it("navigates to the correct route when a tab is clicked", async () => {
    const { wrapper, router } = await mountAt("/settings/general");

    const pushSpy = vi.spyOn(router, "push");
    await wrapper.get('[data-test="settings-tab-hooks"]').trigger("click");

    expect(pushSpy).toHaveBeenCalledWith("/settings/hooks");
  });

  it.each(ALL_TABS)("marks only the '%s' tab as selected when active", async (tab) => {
    const { wrapper } = await mountAt(`/settings/${tab}`);

    for (const t of ALL_TABS) {
      const btn = wrapper.get(`[data-test="settings-tab-${t}"]`);
      const expected = t === tab ? "true" : "false";
      expect(btn.attributes("aria-selected")).toBe(expected);
    }
  });

  // --- ConfigSourceBar visibility ---

  it.each(SOURCE_BAR_TABS)("shows ConfigSourceBar on the '%s' tab", async (tab) => {
    const { wrapper } = await mountAt(`/settings/${tab}`);
    expect(wrapper.find(".settings__source-bar").exists()).toBe(true);
  });

  it.each(["general", "archive"] as const)("hides ConfigSourceBar on the '%s' tab", async (tab) => {
    const { wrapper } = await mountAt(`/settings/${tab}`);
    expect(wrapper.find(".settings__source-bar").exists()).toBe(false);
  });

  it("initializes settings source as user for an ordinary conversation", async () => {
    const { wrapper } = await mountAt("/settings/models", seedOrdinaryConversation);

    const sourceBar = wrapper.get('[data-test="config-source-bar-stub"]');
    expect(sourceBar.attributes("data-initial-source")).toBe("user");
    expect(sourceBar.attributes("data-initial-project-id")).toBe("");
  });

  it("initializes settings source as project for the current project conversation", async () => {
    const { wrapper } = await mountAt("/settings/models", seedProjectConversation);

    const sourceBar = wrapper.get('[data-test="config-source-bar-stub"]');
    expect(sourceBar.attributes("data-initial-source")).toBe("project");
    expect(sourceBar.attributes("data-initial-project-id")).toBe("project-1");
  });

  // --- Accessibility ---

  it("uses role=tablist with an aria-label on the tab container", async () => {
    const { wrapper } = await mountAt();
    const tablist = wrapper.find('[role="tablist"]');
    expect(tablist.exists()).toBe(true);
    expect(tablist.attributes("aria-label")).toBe("Settings sections");
  });

  it("gives every tab button role=tab", async () => {
    const { wrapper } = await mountAt();
    const tablist = wrapper.get('[role="tablist"]');
    const tabs = tablist.findAll('[role="tab"]');
    expect(tabs.length).toBe(ALL_TABS.length);
  });

  // --- Tab switching updates aria-selected ---

  it("updates aria-selected after switching tabs via click", async () => {
    const { wrapper } = await mountAt("/settings/general");

    // General is initially selected
    expect(wrapper.get('[data-test="settings-tab-general"]').attributes("aria-selected")).toBe(
      "true"
    );

    // Click models tab
    await wrapper.get('[data-test="settings-tab-models"]').trigger("click");
    await flushPromises();
    await nextTick();

    expect(wrapper.get('[data-test="settings-tab-models"]').attributes("aria-selected")).toBe(
      "true"
    );
    expect(wrapper.get('[data-test="settings-tab-general"]').attributes("aria-selected")).toBe(
      "false"
    );
  });
});
