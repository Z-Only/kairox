import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import type { ProfileSettingsView } from "@/generated/commands";
import ModelProfileCard from "./ModelProfileCard.vue";
import modelProfileCardSource from "./ModelProfileCard.vue?raw";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    upsertProfileSettings: vi.fn(),
    deleteProfileSettings: vi.fn()
  }
}));

function makeProfile(overrides: Partial<ProfileSettingsView> = {}): ProfileSettingsView {
  return {
    alias: "gpt-4o",
    provider: "openai",
    model_id: "gpt-4o-2024-05-13",
    enabled: true,
    context_window: 128000,
    output_limit: 4096,
    temperature: 0.7,
    top_p: null,
    top_k: null,
    max_tokens: 4096,
    base_url: null,
    api_key_env: "OPENAI_API_KEY",
    has_api_key: true,
    writable: true,
    config_path: "/home/user/.config/kairox/profiles.toml",
    source: "profiles_toml",
    ...overrides
  };
}

function mountCard(
  profile: ProfileSettingsView,
  opts: { index?: number; total?: number; busyAlias?: string | null } = {}
) {
  const mountOptions: MountWithPluginsOptions<typeof ModelProfileCard> = {
    reusePinia: true,
    mount: {
      props: {
        profile,
        index: opts.index ?? 0,
        total: opts.total ?? 3,
        busyAlias: opts.busyAlias ?? null
      }
    }
  };
  return mountWithPlugins(ModelProfileCard, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("ModelProfileCard", () => {
  describe("source migration guard", () => {
    it("uses shared summary/status chrome without duplicating the effective audit row", () => {
      expectSourceMigration(modelProfileCardSource, {
        required: ["SettingsCardItem", "SettingsItemSummary", "SettingsStatusTag"],
        forbidden: ["SettingsEffectiveAudit", "model-audit-"]
      });
    });
  });

  describe("rendering profile info", () => {
    it("renders alias as title", () => {
      const wrapper = mountCard(makeProfile({ alias: "my-model" }));
      expect(wrapper.text()).toContain("my-model");
    });

    it("renders provider and model_id in description", () => {
      const wrapper = mountCard(makeProfile({ provider: "anthropic", model_id: "claude-3-opus" }));
      expect(wrapper.text()).toContain("anthropic");
      expect(wrapper.text()).toContain("claude-3-opus");
    });

    it("renders data-test attribute with alias", () => {
      const wrapper = mountCard(makeProfile({ alias: "test-alias" }));
      expect(wrapper.find('[data-test="model-row-test-alias"]').exists()).toBe(true);
    });
  });

  describe("source tag", () => {
    it("renders source label for profiles_toml", () => {
      const wrapper = mountCard(makeProfile({ source: "profiles_toml" }));
      // The sourceLabel maps "profiles_toml" to i18n key "models.sourceProfilesToml"
      expect(wrapper.text()).toContain("profiles.toml");
    });

    it("renders source label for defaults", () => {
      const wrapper = mountCard(makeProfile({ source: "defaults" }));
      expect(wrapper.text()).toMatch(/built.?in|default/i);
    });

    it("renders source label for user_config", () => {
      const wrapper = mountCard(makeProfile({ source: "user_config" }));
      expect(wrapper.text()).toMatch(/user/i);
    });

    it("renders source label for project_config", () => {
      const wrapper = mountCard(makeProfile({ source: "project_config" }));
      expect(wrapper.text()).toMatch(/project/i);
    });

    it("renders raw source string for unknown sources", () => {
      const wrapper = mountCard(makeProfile({ source: "custom_source" }));
      expect(wrapper.text()).toContain("custom_source");
    });
  });

  describe("enabled/disabled tag", () => {
    it("renders enabled tag when profile is enabled", () => {
      const wrapper = mountCard(makeProfile({ enabled: true }));
      expect(wrapper.text()).toContain("Enabled");
    });

    it("renders disabled tag when profile is disabled", () => {
      const wrapper = mountCard(makeProfile({ enabled: false }));
      expect(wrapper.text()).toContain("Disabled");
    });
  });

  describe("parameter display", () => {
    it("renders context window when present", () => {
      const wrapper = mountCard(makeProfile({ context_window: 128000 }));
      expect(wrapper.text()).toContain("128,000");
    });

    it("renders output limit when present", () => {
      const wrapper = mountCard(makeProfile({ output_limit: 4096 }));
      expect(wrapper.text()).toContain("4,096");
    });

    it("renders temperature when present", () => {
      const wrapper = mountCard(makeProfile({ temperature: 0.7 }));
      expect(wrapper.text()).toContain("0.7");
    });

    it("does not render temperature tag when null", () => {
      const wrapper = mountCard(makeProfile({ temperature: null }));
      expect(wrapper.text()).not.toMatch(/temperature.*null/i);
    });
  });

  describe("enable/disable toggle", () => {
    it("shows Disable text when profile is enabled", () => {
      const wrapper = mountCard(makeProfile({ enabled: true }));
      const btn = wrapper.find('[data-test="model-enable-gpt-4o"]');
      expect(btn.exists()).toBe(true);
      expect(btn.text()).toContain("Disable");
    });

    it("shows Enable text when profile is disabled", () => {
      const wrapper = mountCard(makeProfile({ enabled: false }));
      const btn = wrapper.find('[data-test="model-enable-gpt-4o"]');
      expect(btn.text()).toContain("Enable");
    });

    it("emits toggle event on click", async () => {
      const profile = makeProfile({ enabled: true });
      const wrapper = mountCard(profile);
      await wrapper.find('[data-test="model-enable-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("toggle")).toBeTruthy();
      expect(wrapper.emitted("toggle")![0]).toEqual([profile]);
    });
  });

  describe("edit action", () => {
    it("renders edit button", () => {
      const wrapper = mountCard(makeProfile());
      const btn = wrapper.find('[data-test="model-edit-gpt-4o"]');
      expect(btn.exists()).toBe(true);
    });

    it("emits edit event on click", async () => {
      const profile = makeProfile();
      const wrapper = mountCard(profile);
      await wrapper.find('[data-test="model-edit-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("edit")).toBeTruthy();
      expect(wrapper.emitted("edit")![0]).toEqual([profile]);
    });
  });

  describe("delete action", () => {
    it("renders delete button when writable", () => {
      const wrapper = mountCard(makeProfile({ writable: true }));
      expect(wrapper.find('[data-test="model-delete-gpt-4o"]').exists()).toBe(true);
    });

    it("does not render delete button when not writable", () => {
      const wrapper = mountCard(makeProfile({ writable: false }));
      expect(wrapper.find('[data-test="model-delete-gpt-4o"]').exists()).toBe(false);
    });

    it("emits remove event with alias on click", async () => {
      const wrapper = mountCard(makeProfile({ alias: "gpt-4o", writable: true }));
      await wrapper.find('[data-test="model-delete-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("remove")).toBeTruthy();
      expect(wrapper.emitted("remove")![0]).toEqual(["gpt-4o"]);
    });
  });

  describe("test connectivity action", () => {
    it("renders test connectivity button", () => {
      const wrapper = mountCard(makeProfile());
      const btn = wrapper.find('[data-test="model-test-gpt-4o"]');
      expect(btn.exists()).toBe(true);
    });

    it("emits test event on click", async () => {
      const profile = makeProfile();
      const wrapper = mountCard(profile);
      await wrapper.find('[data-test="model-test-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("test")).toBeTruthy();
      expect(wrapper.emitted("test")![0]).toEqual([profile]);
    });
  });

  describe("reorder actions", () => {
    it("renders move up and move down buttons", () => {
      const wrapper = mountCard(makeProfile(), { index: 1, total: 3 });
      expect(wrapper.find('[data-test="model-move-up-gpt-4o"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="model-move-down-gpt-4o"]').exists()).toBe(true);
    });

    it("disables move up button when index is 0", () => {
      const wrapper = mountCard(makeProfile(), { index: 0, total: 3 });
      const btn = wrapper.find('[data-test="model-move-up-gpt-4o"]');
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("disables move down button when index is last", () => {
      const wrapper = mountCard(makeProfile(), { index: 2, total: 3 });
      const btn = wrapper.find('[data-test="model-move-down-gpt-4o"]');
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("enables both buttons when index is in the middle", () => {
      const wrapper = mountCard(makeProfile(), { index: 1, total: 3 });
      const upBtn = wrapper.find('[data-test="model-move-up-gpt-4o"]');
      const downBtn = wrapper.find('[data-test="model-move-down-gpt-4o"]');
      expect((upBtn.element as HTMLButtonElement).disabled).toBe(false);
      expect((downBtn.element as HTMLButtonElement).disabled).toBe(false);
    });

    it("emits move event with direction -1 on move up click", async () => {
      const wrapper = mountCard(makeProfile(), { index: 1, total: 3 });
      await wrapper.find('[data-test="model-move-up-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("move")).toBeTruthy();
      expect(wrapper.emitted("move")![0]).toEqual(["gpt-4o", -1]);
    });

    it("emits move event with direction 1 on move down click", async () => {
      const wrapper = mountCard(makeProfile(), { index: 1, total: 3 });
      await wrapper.find('[data-test="model-move-down-gpt-4o"]').trigger("click");
      expect(wrapper.emitted("move")).toBeTruthy();
      expect(wrapper.emitted("move")![0]).toEqual(["gpt-4o", 1]);
    });
  });

  describe("busy state", () => {
    it("disables all action buttons when busyAlias matches profile alias", () => {
      const wrapper = mountCard(makeProfile({ alias: "gpt-4o" }), {
        index: 1,
        total: 3,
        busyAlias: "gpt-4o"
      });
      const editBtn = wrapper.find('[data-test="model-edit-gpt-4o"]');
      const enableBtn = wrapper.find('[data-test="model-enable-gpt-4o"]');
      const testBtn = wrapper.find('[data-test="model-test-gpt-4o"]');
      const moveUpBtn = wrapper.find('[data-test="model-move-up-gpt-4o"]');
      const moveDownBtn = wrapper.find('[data-test="model-move-down-gpt-4o"]');
      expect((editBtn.element as HTMLButtonElement).disabled).toBe(true);
      expect((enableBtn.element as HTMLButtonElement).disabled).toBe(true);
      expect((testBtn.element as HTMLButtonElement).disabled).toBe(true);
      expect((moveUpBtn.element as HTMLButtonElement).disabled).toBe(true);
      expect((moveDownBtn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("does not disable buttons when busyAlias differs from profile alias", () => {
      const wrapper = mountCard(makeProfile({ alias: "gpt-4o" }), {
        index: 1,
        total: 3,
        busyAlias: "other-model"
      });
      const editBtn = wrapper.find('[data-test="model-edit-gpt-4o"]');
      expect((editBtn.element as HTMLButtonElement).disabled).toBe(false);
    });
  });

  describe("effective audit", () => {
    it("does not render a duplicate audit element below summary tags", () => {
      const wrapper = mountCard(makeProfile({ alias: "gpt-4o" }));
      expect(wrapper.find('[data-test="model-audit-gpt-4o"]').exists()).toBe(false);
    });
  });
});
