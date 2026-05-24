import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { commands, type AgentSettingsView } from "@/generated/commands";
import AgentSettingsPane from "./AgentSettingsPane.vue";
import agentSettingsPaneSource from "./AgentSettingsPane.vue?raw";

vi.mock("@/generated/commands", () => ({
  commands: {
    listAgentSettings: vi.fn(),
    upsertAgentSettings: vi.fn(),
    deleteAgentSettings: vi.fn(),
    copyAgentSettings: vi.fn(),
    openAgentsDir: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

const workerAgent: AgentSettingsView = {
  settingsId: "Builtin:worker",
  name: "worker",
  description: "Execution-focused agent.",
  scope: "Builtin",
  path: "builtin://worker",
  tools: [],
  modelProfile: null,
  permissionMode: "workspace_write",
  skills: [],
  nicknameCandidates: ["Worker"],
  enabled: true,
  instructions: "Implement scoped changes.",
  effective: true,
  shadowedBy: null,
  valid: true,
  validationError: null,
  editable: false,
  deletable: false
};

const reviewerAgent: AgentSettingsView = {
  settingsId: "User:code-reviewer",
  name: "code-reviewer",
  description: "Review code.",
  scope: "User",
  path: "/home/.config/kairox/agents/code-reviewer.md",
  tools: ["fs.read", "search"],
  modelProfile: "fast",
  permissionMode: "read_only",
  skills: ["kairox-dev-workflow"],
  nicknameCandidates: ["Reviewer"],
  enabled: true,
  instructions: "Lead with findings.",
  effective: true,
  shadowedBy: null,
  valid: true,
  validationError: null,
  editable: true,
  deletable: true
};

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function mountPane() {
  return mountWithPlugins(AgentSettingsPane, { reusePinia: true }).wrapper;
}

function renderedAgentIds(wrapper: ReturnType<typeof mountPane>): string[] {
  return wrapper
    .findAll("[data-agent-settings-id]")
    .map((row) => row.attributes("data-agent-settings-id"));
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listAgentSettings.mockResolvedValue(ok([workerAgent, reviewerAgent]));
  mockedCommands.upsertAgentSettings.mockResolvedValue(ok(reviewerAgent));
  mockedCommands.deleteAgentSettings.mockResolvedValue(ok(null));
  mockedCommands.copyAgentSettings.mockResolvedValue(ok({ ...workerAgent, scope: "User" }));
  mockedCommands.openAgentsDir.mockResolvedValue(ok("/home/.config/kairox/agents"));
});

describe("AgentSettingsPane", () => {
  it("renders built-in and user agents", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="agent-row-worker"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="agent-row-code-reviewer"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="agent-search-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="agent-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="agent-row-worker"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find('[data-test="agent-row-worker"]').text()).toContain("Built-in");
    expect(wrapper.find('[data-test="agent-row-code-reviewer"]').text()).toContain("fast");
  });

  it("filters agents by search text", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-search-input"]').setValue("review");

    expect(wrapper.find('[data-test="agent-row-code-reviewer"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="agent-row-worker"]').exists()).toBe(false);
  });

  it("matches search against agent metadata", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-search-input"]').setValue("workspace_write");

    expect(wrapper.find('[data-test="agent-row-worker"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="agent-row-code-reviewer"]').exists()).toBe(false);
  });

  it("sorts the filtered agent list by selected order", async () => {
    mockedCommands.listAgentSettings.mockResolvedValueOnce(
      ok([
        {
          ...workerAgent,
          settingsId: "User:zeta-builder",
          name: "zeta-builder",
          scope: "User",
          description: "Build focused agent."
        },
        {
          ...workerAgent,
          settingsId: "Builtin:worker",
          name: "worker",
          scope: "Builtin",
          description: "Execution focused agent."
        },
        {
          ...reviewerAgent,
          settingsId: "Project:alpha-builder",
          name: "alpha-builder",
          scope: "Project",
          description: "Build focused agent."
        }
      ])
    );
    const wrapper = mountPane();
    await flushPromises();

    const sortSelect = wrapper.find<HTMLSelectElement>('[data-test="agent-sort-select"]');
    expect(sortSelect.exists()).toBe(true);
    expect(sortSelect.attributes("aria-label")).toBe("Agent sort");

    await wrapper.find('[data-test="agent-search-input"]').setValue("build");
    expect(renderedAgentIds(wrapper)).toEqual(["User:zeta-builder", "Project:alpha-builder"]);

    await sortSelect.setValue("name");
    expect(renderedAgentIds(wrapper)).toEqual(["Project:alpha-builder", "User:zeta-builder"]);
  });

  it("shows a filtered empty state when no agents match search", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-search-input"]').setValue("does-not-exist");

    const empty = wrapper.find('[data-test="agent-filter-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No agents match your search.");
    expect(wrapper.find('[data-test="agent-list"]').exists()).toBe(false);
  });

  it("renders a consistent effective-state audit for shadowed agents", async () => {
    mockedCommands.listAgentSettings.mockResolvedValueOnce(
      ok([
        {
          ...workerAgent,
          effective: false,
          shadowedBy: "User:worker"
        },
        reviewerAgent
      ])
    );

    const wrapper = mountPane();
    await flushPromises();

    const audit = wrapper.find('[data-test="agent-audit-worker-builtin"]');
    expect(audit.exists()).toBe(true);
    expect(audit.text()).toContain("Source");
    expect(audit.text()).toContain("Built-in");
    expect(audit.text()).toContain("State");
    expect(audit.text()).toContain("Enabled");
    expect(audit.text()).toContain("Effective");
    expect(audit.text()).toContain("Shadowed by User:worker");
    expect(audit.text()).toContain("Validity");
    expect(audit.text()).toContain("Valid");
  });

  it("loads selected agent into the editor and saves changes", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-edit-code-reviewer"]').trigger("click");
    await flushPromises();
    expect(wrapper.find<HTMLInputElement>('[data-test="agent-form-name"]').element.value).toBe(
      "code-reviewer"
    );
    await wrapper
      .find<HTMLInputElement>('[data-test="agent-form-description"]')
      .setValue("Review diffs.");
    await wrapper.find('[data-test="agent-save"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.upsertAgentSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        scope: "User",
        name: "code-reviewer",
        description: "Review diffs.",
        instructions: "Lead with findings."
      })
    );
  });

  it("copies a built-in agent to user scope", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-copy-worker"]').trigger("click");

    expect(mockedCommands.copyAgentSettings).toHaveBeenCalledWith("Builtin:worker", "User");
  });

  it("deletes writable agents", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="agent-delete-code-reviewer"]').trigger("click");

    expect(mockedCommands.deleteAgentSettings).toHaveBeenCalledWith("User:code-reviewer");
  });

  it("uses shared settings state chrome when no agents are configured", async () => {
    mockedCommands.listAgentSettings.mockResolvedValueOnce(ok([]));

    const wrapper = mountPane();
    await flushPromises();

    const empty = wrapper.find('[data-test="agent-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.text()).toContain("No agents configured.");
  });

  it("does not keep local agent row chrome after moving to SettingsCardItem", () => {
    expectSourceMigration(agentSettingsPaneSource, {
      required: [
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsItemSummary",
        "SettingsItemMeta",
        "SettingsStatusTag"
      ],
      forbidden: [
        ".agent-row {",
        ".agent-row__title",
        ".agent-row__meta",
        "tag-success",
        "tag-warning",
        "tag-error",
        "border-bottom: 1px solid var(--app-border-color)"
      ]
    });
  });

  it("uses shared settings toolbar instead of local agent toolbar chrome", () => {
    expectSourceMigration(agentSettingsPaneSource, {
      required: ["SettingsToolbar", "SettingsFilterBar"],
      forbidden: ['class="agent-settings__toolbar"', ".agent-settings__toolbar,"]
    });
  });

  it("uses shared form fields and controls in the agent editor", () => {
    expectSourceMigration(agentSettingsPaneSource, {
      required: ["KxFormField", "KxInput", "KxTextarea", 'data-test="agent-form-instructions"'],
      forbidden: ["kx-form-control", ".agent-editor input,", ".agent-editor textarea {"]
    });
  });

  it("does not keep agent editor placeholder examples inline in the component source", () => {
    expectSourceMigration(agentSettingsPaneSource, {
      forbidden: [
        'placeholder="code-reviewer"',
        'placeholder="fs.read, search, shell"',
        'placeholder="kairox-dev-workflow"',
        'placeholder="Reviewer, Audit"'
      ]
    });
  });
});
