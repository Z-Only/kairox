import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import { setActivePinia, createPinia } from "pinia";
import { createI18n } from "vue-i18n";
import en from "@/locales/en.json";
import { useUiStore } from "@/stores/ui";
import TrajectoryViewer from "./TrajectoryViewer.vue";

// Mock the generated commands module
const mockListTrajectories = vi.fn();
const mockGetTrajectorySteps = vi.fn();
const mockExportTrajectory = vi.fn();

vi.mock("@/generated/commands", () => ({
  commands: {
    listTrajectories: (...args: unknown[]) => mockListTrajectories(...args),
    getTrajectorySteps: (...args: unknown[]) => mockGetTrajectorySteps(...args),
    exportTrajectory: (...args: unknown[]) => mockExportTrajectory(...args)
  }
}));

// Mock session store — use reactive() so Vue's watch() can track property changes
const sessionState = reactive({
  currentSessionId: null as string | null
});

vi.mock("@/stores/session", () => ({
  useSessionStore: () => sessionState
}));

function createTestI18n() {
  return createI18n({
    legacy: false,
    locale: "en",
    fallbackLocale: "en",
    messages: { en }
  });
}

function mountViewer() {
  return mount(TrajectoryViewer, {
    global: {
      plugins: [createTestI18n()],
      stubs: {
        KxEmptyState: {
          template: '<div class="kx-empty-state" data-test="empty-state"><slot /></div>',
          props: ["compact"]
        },
        KxButton: {
          template:
            '<button class="kx-button" data-test="kx-button" @click="$emit(\'click\', $event)"><slot /></button>',
          props: ["size", "variant"],
          emits: ["click"]
        }
      }
    }
  });
}

describe("TrajectoryViewer", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    sessionState.currentSessionId = null;
    mockListTrajectories.mockReset();
    mockGetTrajectorySteps.mockReset();
    mockExportTrajectory.mockReset();
  });

  describe("empty states", () => {
    it("shows no-session message when currentSessionId is null", () => {
      sessionState.currentSessionId = null;
      const wrapper = mountViewer();
      expect(wrapper.find('[data-test="empty-state"]').exists()).toBe(true);
      expect(wrapper.text()).toContain(en.trajectory.noSession);
    });

    it("shows loading state while fetching trajectories", async () => {
      sessionState.currentSessionId = "ses_1";
      // Never resolve to keep loading state
      mockListTrajectories.mockReturnValue(new Promise(() => {}));
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain(en.trajectory.loading);
    });

    it("shows error message on fetch failure", async () => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({
        status: "error",
        error: "Connection refused"
      });
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain("Connection refused");
    });

    it("shows error message on exception", async () => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockRejectedValue(new Error("Network error"));
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain("Network error");
    });

    it("shows empty message when trajectories list is empty", async () => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({ status: "ok", data: [] });
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain(en.trajectory.empty);
    });
  });

  describe("trajectory list rendering", () => {
    const sampleTrajectories = [
      {
        trajectory_id: "traj_1",
        task_id: "task_analyze",
        outcome: "success",
        step_count: 5,
        started_at: "2026-06-01T10:00:00Z",
        completed_at: "2026-06-01T10:01:30Z"
      },
      {
        trajectory_id: "traj_2",
        task_id: "task_fix_bug",
        outcome: "failed",
        step_count: 3,
        started_at: "2026-06-01T11:00:00Z",
        completed_at: null
      },
      {
        trajectory_id: "traj_3",
        task_id: "task_still_running",
        outcome: "in_progress",
        step_count: 1,
        started_at: "2026-06-01T12:00:00Z",
        completed_at: null
      }
    ];

    beforeEach(() => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({ status: "ok", data: sampleTrajectories });
    });

    it("renders trajectory cards after successful fetch", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      const cards = wrapper.findAll('[data-test="trajectory-card"]');
      expect(cards.length).toBe(3);
    });

    it("displays task_id and outcome for each trajectory", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      const cards = wrapper.findAll('[data-test="trajectory-card"]');
      expect(cards[0].text()).toContain("task_analyze");
      expect(cards[0].text()).toContain(en.trajectory.outcome.success);
      expect(cards[1].text()).toContain("task_fix_bug");
      expect(cards[1].text()).toContain(en.trajectory.outcome.failed);
    });

    it("renders a compact outcome summary above the trajectory list", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const summary = wrapper.get('[data-test="trajectory-summary"]');
      expect(summary.text()).toContain("3 trajectories");
      expect(summary.text()).toContain("1 succeeded");
      expect(summary.text()).toContain("1 failed");
      expect(summary.text()).toContain("1 in progress");
      expect(summary.text()).not.toContain("0 cancelled");
    });

    it("applies correct badge class for each outcome", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      const badges = wrapper.findAll(".trajectory-badge");
      expect(badges[0].classes()).toContain("badge--success");
      expect(badges[1].classes()).toContain("badge--failed");
    });

    it("displays step count", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();
      const cards = wrapper.findAll('[data-test="trajectory-card"]');
      expect(cards[0].text()).toContain("5");
    });
  });

  describe("trajectory expansion", () => {
    const sampleTrajectory = {
      trajectory_id: "traj_1",
      task_id: "task_analyze",
      outcome: "success",
      step_count: 2,
      started_at: "2026-06-01T10:00:00Z",
      completed_at: "2026-06-01T10:01:30Z"
    };

    const sampleSteps = [
      {
        step_index: 0,
        action: "shell.exec",
        action_input: "ls -la /tmp/project",
        observation: "total 42\ndrwxr-xr-x 3 user group 96 Jun 1 10:00 .",
        duration_ms: 150,
        timestamp: "2026-06-01T10:00:01Z"
      },
      {
        step_index: 1,
        action: "fs.read",
        action_input: "/tmp/project/src/main.rs",
        observation:
          'fn main() {\n    println!("Hello, world!");\n}\n// this is a long observation that should be truncated when collapsed because it exceeds the default truncation length of 120 characters total',
        duration_ms: 2500,
        timestamp: "2026-06-01T10:00:02Z"
      }
    ];

    beforeEach(() => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({ status: "ok", data: [sampleTrajectory] });
      mockGetTrajectorySteps.mockResolvedValue({ status: "ok", data: sampleSteps });
    });

    it("expands trajectory steps on card click", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const card = wrapper.find('[data-test="trajectory-card"]');
      await card.trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const steps = wrapper.findAll('[data-test="trajectory-step"]');
      expect(steps.length).toBe(2);
      expect(mockGetTrajectorySteps).toHaveBeenCalledWith("traj_1");
    });

    it("collapses trajectory steps on second click", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const card = wrapper.find('[data-test="trajectory-card"]');
      await card.trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="trajectory-step"]').length).toBe(2);

      await card.trigger("click");
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="trajectory-step"]').length).toBe(0);
    });

    it("displays step action, duration and index", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const firstStep = wrapper.findAll('[data-test="trajectory-step"]')[0];
      expect(firstStep.text()).toContain("#0");
      expect(firstStep.text()).toContain("shell.exec");
      expect(firstStep.text()).toContain("150ms");
    });

    it("formats duration in seconds when >= 1000ms", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const secondStep = wrapper.findAll('[data-test="trajectory-step"]')[1];
      expect(secondStep.text()).toContain("2.5s");
    });

    it("truncates long text fields and expands on click", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const steps = wrapper.findAll('[data-test="trajectory-step"]');
      const secondStep = steps[1];
      const observationField = secondStep.findAll(".step-field")[1];

      // Should be truncated initially (ends with ellipsis)
      expect(observationField.find(".step-field-value").text()).toContain("…");

      // Click to expand
      await observationField.trigger("click");
      await wrapper.vm.$nextTick();

      // Should now contain full text without ellipsis truncation
      expect(observationField.find(".step-field-value").text()).toContain(
        "this is a long observation"
      );
    });

    it("toggles input expansion independently", async () => {
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const steps = wrapper.findAll('[data-test="trajectory-step"]');
      const firstStep = steps[0];
      const inputField = firstStep.findAll(".step-field")[0];

      // Click to expand then collapse
      await inputField.trigger("click");
      await wrapper.vm.$nextTick();
      await inputField.trigger("click");
      await wrapper.vm.$nextTick();

      // Should be truncated again (short enough to not show ellipsis)
      expect(inputField.find(".step-field-value").text()).toBe("ls -la /tmp/project");
    });

    it("shows empty steps message when API returns empty array", async () => {
      mockGetTrajectorySteps.mockResolvedValue({ status: "ok", data: [] });
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toContain(en.trajectory.noSteps);
    });

    it("handles steps fetch error gracefully", async () => {
      mockGetTrajectorySteps.mockResolvedValue({ status: "error", error: "Not found" });
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      // Should show noSteps (empty steps array)
      expect(wrapper.text()).toContain(en.trajectory.noSteps);
    });

    it("handles steps fetch exception gracefully", async () => {
      mockGetTrajectorySteps.mockRejectedValue(new Error("timeout"));
      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="trajectory-card"]').trigger("click");
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toContain(en.trajectory.noSteps);
    });
  });

  describe("export trajectory", () => {
    const sampleTrajectory = {
      trajectory_id: "traj_export",
      task_id: "task_export_test",
      outcome: "success",
      step_count: 1,
      started_at: "2026-06-01T10:00:00Z",
      completed_at: "2026-06-01T10:00:05Z"
    };

    beforeEach(() => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({ status: "ok", data: [sampleTrajectory] });
    });

    it("copies exported trajectory to clipboard on export button click", async () => {
      const ui = useUiStore();
      const writeText = vi.fn().mockResolvedValue(undefined);
      Object.assign(navigator, { clipboard: { writeText } });
      mockExportTrajectory.mockResolvedValue({ status: "ok", data: '{"steps":[]}' });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const exportBtn = wrapper.find('[data-test="trajectory-export"]');
      await exportBtn.trigger("click");
      await wrapper.vm.$nextTick();

      expect(mockExportTrajectory).toHaveBeenCalledWith("traj_export");
      expect(writeText).toHaveBeenCalledWith('{"steps":[]}');
      expect(ui.toasts.at(-1)).toMatchObject({
        message: en.notifications.copySuccess,
        type: "success"
      });
    });

    it("does not propagate click to card when clicking export", async () => {
      mockExportTrajectory.mockResolvedValue({ status: "ok", data: "{}" });
      Object.assign(navigator, { clipboard: { writeText: vi.fn() } });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const exportBtn = wrapper.find('[data-test="trajectory-export"]');
      await exportBtn.trigger("click");
      await wrapper.vm.$nextTick();

      // Should not expand trajectory (getTrajectorySteps should not be called)
      expect(mockGetTrajectorySteps).not.toHaveBeenCalled();
    });

    it("shows an error toast when export fails", async () => {
      const ui = useUiStore();
      mockExportTrajectory.mockResolvedValue({ status: "error", error: "export failed" });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const exportBtn = wrapper.find('[data-test="trajectory-export"]');
      await exportBtn.trigger("click");
      await wrapper.vm.$nextTick();

      expect(ui.toasts.at(-1)).toMatchObject({
        message: `${en.notifications.copyFailed}: Error: export failed`,
        type: "error"
      });
    });
  });

  describe("session change reactivity", () => {
    it("refetches trajectories when session changes", async () => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({ status: "ok", data: [] });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(mockListTrajectories).toHaveBeenCalledWith("ses_1");

      sessionState.currentSessionId = "ses_2";
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(mockListTrajectories).toHaveBeenCalledWith("ses_2");
    });

    it("clears trajectories when session becomes null", async () => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({
        status: "ok",
        data: [
          {
            trajectory_id: "traj_1",
            task_id: "task_1",
            outcome: "success",
            step_count: 1,
            started_at: "2026-06-01T10:00:00Z",
            completed_at: "2026-06-01T10:00:05Z"
          }
        ]
      });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="trajectory-card"]').length).toBe(1);

      sessionState.currentSessionId = null;
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="trajectory-card"]').length).toBe(0);
      expect(wrapper.text()).toContain(en.trajectory.noSession);
    });
  });

  describe("outcome badge classes", () => {
    it.each([
      ["success", "badge--success"],
      ["failed", "badge--failed"],
      ["cancelled", "badge--cancelled"],
      ["in_progress", "badge--in-progress"]
    ])("applies %s outcome as %s class", async (outcome, expectedClass) => {
      sessionState.currentSessionId = "ses_1";
      mockListTrajectories.mockResolvedValue({
        status: "ok",
        data: [
          {
            trajectory_id: `traj_${outcome}`,
            task_id: `task_${outcome}`,
            outcome,
            step_count: 1,
            started_at: "2026-06-01T10:00:00Z",
            completed_at: "2026-06-01T10:00:05Z"
          }
        ]
      });

      const wrapper = mountViewer();
      await wrapper.vm.$nextTick();
      await wrapper.vm.$nextTick();

      const badge = wrapper.find(".trajectory-badge");
      expect(badge.classes()).toContain(expectedClass);
      expect(badge.text()).toBe(
        en.trajectory.outcome[outcome as keyof typeof en.trajectory.outcome]
      );
    });
  });
});
