import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
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
    expect(wrapper.find('[data-test="agent-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="agent-row-worker"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find('[data-test="agent-row-worker"]').text()).toContain("Built-in");
    expect(wrapper.find('[data-test="agent-row-code-reviewer"]').text()).toContain("fast");
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
    expect(agentSettingsPaneSource).toContain("SettingsCardList");
    expect(agentSettingsPaneSource).toContain("SettingsCardItem");
    expect(agentSettingsPaneSource).toContain("SettingsItemSummary");
    expect(agentSettingsPaneSource).toContain("SettingsItemMeta");
    expect(agentSettingsPaneSource).not.toContain(".agent-row {");
    expect(agentSettingsPaneSource).not.toContain(".agent-row__title");
    expect(agentSettingsPaneSource).not.toContain(".agent-row__meta");
    expect(agentSettingsPaneSource).not.toContain(
      "border-bottom: 1px solid var(--app-border-color)"
    );
  });

  it("uses shared settings toolbar instead of local agent toolbar chrome", () => {
    expect(agentSettingsPaneSource).toContain("SettingsToolbar");
    expect(agentSettingsPaneSource).not.toContain('class="agent-settings__toolbar"');
    expect(agentSettingsPaneSource).not.toContain(".agent-settings__toolbar,");
  });

  it("uses shared form fields and controls in the agent editor", () => {
    expect(agentSettingsPaneSource).toContain("KxFormField");
    expect(agentSettingsPaneSource).toContain("KxInput");
    expect(agentSettingsPaneSource).toContain("KxTextarea");
    expect(agentSettingsPaneSource).toContain('data-test="agent-form-instructions"');
    expect(agentSettingsPaneSource).not.toContain("kx-form-control");
    expect(agentSettingsPaneSource).not.toContain(".agent-editor input,");
    expect(agentSettingsPaneSource).not.toContain(".agent-editor textarea {");
  });
});
