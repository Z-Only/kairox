import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import AutonomousSettingsPane from "./AutonomousSettingsPane.vue";
import { useAutonomousStore } from "@/stores/autonomous";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

const taskActive = {
  autonomous_task_id: "task_1",
  workspace_id: "ws_1",
  goal: "Implement feature X with tests and documentation",
  state: "active",
  current_session_id: "ses_1",
  session_count: 2,
  max_sessions: 5,
  created_at: "2026-06-01T10:00:00Z",
  updated_at: "2026-06-01T12:00:00Z"
};

const taskPaused = {
  autonomous_task_id: "task_2",
  workspace_id: "ws_1",
  goal: "Refactor module Y",
  state: "paused",
  current_session_id: "ses_2",
  session_count: 1,
  max_sessions: 3,
  created_at: "2026-06-02T09:00:00Z",
  updated_at: "2026-06-02T09:30:00Z"
};

const taskCompleted = {
  autonomous_task_id: "task_3",
  workspace_id: "ws_1",
  goal: "Fix bug Z",
  state: "completed",
  current_session_id: "ses_3",
  session_count: 1,
  max_sessions: 3,
  created_at: "2026-06-03T08:00:00Z",
  updated_at: "2026-06-03T08:15:00Z"
};

const checkpoint1 = {
  checkpoint_id: "cp_1",
  session_id: "ses_1",
  session_index: 0,
  completed_items: ["Added unit tests", "Updated docs"],
  remaining_items: ["Integration tests"],
  git_sha: "abc12345def67890",
  end_reason: "checkpoint",
  created_at: "2026-06-01T11:00:00Z"
};

const checkpoint2 = {
  checkpoint_id: "cp_2",
  session_id: "ses_2",
  session_index: 1,
  completed_items: ["Refactored core module"],
  remaining_items: ["Update imports", "Run tests"],
  git_sha: null,
  end_reason: "max_turns",
  created_at: "2026-06-01T12:00:00Z"
};

function mountPane() {
  return mountWithPlugins(AutonomousSettingsPane);
}

describe("AutonomousSettingsPane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
    mockedInvoke.mockResolvedValue([]);
  });

  it("shows empty state when no tasks", async () => {
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-task-list"]').exists()).toBe(false);
  });

  it("renders task list after fetching", async () => {
    mockedInvoke.mockResolvedValueOnce([taskActive, taskPaused, taskCompleted]);
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-task-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-empty"]').exists()).toBe(false);

    const cards = wrapper.findAll(".autonomous-pane__card");
    expect(cards.length).toBe(3);
  });

  it("displays task goal and state", async () => {
    mockedInvoke.mockResolvedValueOnce([taskActive]);
    const wrapper = mountPane();
    await flushPromises();

    const card = wrapper.find(`[data-test="autonomous-task-${taskActive.autonomous_task_id}"]`);
    expect(card.text()).toContain("Implement feature X");
    expect(card.text()).toContain("active");
    expect(card.text()).toContain("2/5");
  });

  it("shows pause button for active tasks", async () => {
    mockedInvoke.mockResolvedValueOnce([taskActive]);
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-pause-btn"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-resume-btn"]').exists()).toBe(false);
  });

  it("shows resume button for paused tasks", async () => {
    mockedInvoke.mockResolvedValueOnce([taskPaused]);
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-resume-btn"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-pause-btn"]').exists()).toBe(false);
  });

  it("shows cancel button for active and paused tasks", async () => {
    mockedInvoke.mockResolvedValueOnce([taskActive, taskPaused, taskCompleted]);
    const wrapper = mountPane();
    await flushPromises();

    const cancelButtons = wrapper.findAll('[data-test="autonomous-cancel-btn"]');
    expect(cancelButtons.length).toBe(2);
  });

  it("does not show action buttons for completed tasks", async () => {
    mockedInvoke.mockResolvedValueOnce([taskCompleted]);
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-pause-btn"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="autonomous-resume-btn"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="autonomous-cancel-btn"]').exists()).toBe(false);
  });

  it("selects a task and shows detail panel", async () => {
    mockedInvoke
      .mockResolvedValueOnce([taskActive])
      .mockResolvedValueOnce([checkpoint1, checkpoint2]);

    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-detail"]').exists()).toBe(false);

    await wrapper
      .find(`[data-test="autonomous-task-${taskActive.autonomous_task_id}"]`)
      .trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-detail"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-detail"]').text()).toContain(taskActive.goal);
  });

  it("deselects a task on second click", async () => {
    mockedInvoke.mockResolvedValueOnce([taskActive]).mockResolvedValueOnce([checkpoint1]);

    const wrapper = mountPane();
    await flushPromises();

    const card = wrapper.find(`[data-test="autonomous-task-${taskActive.autonomous_task_id}"]`);
    await card.trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="autonomous-detail"]').exists()).toBe(true);

    await card.trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="autonomous-detail"]').exists()).toBe(false);
  });

  it("displays checkpoints when a task is selected", async () => {
    mockedInvoke
      .mockResolvedValueOnce([taskActive])
      .mockResolvedValueOnce([checkpoint1, checkpoint2]);

    const wrapper = mountPane();
    await flushPromises();

    await wrapper
      .find(`[data-test="autonomous-task-${taskActive.autonomous_task_id}"]`)
      .trigger("click");
    await flushPromises();

    const checkpointsEl = wrapper.find('[data-test="autonomous-checkpoints"]');
    expect(checkpointsEl.exists()).toBe(true);
    expect(checkpointsEl.text()).toContain("Added unit tests");
    expect(checkpointsEl.text()).toContain("Integration tests");
    expect(checkpointsEl.text()).toContain("abc12345");
  });

  it("calls pause and refreshes tasks", async () => {
    mockedInvoke
      .mockResolvedValueOnce([taskActive])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([{ ...taskActive, state: "paused" }]);

    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="autonomous-pause-btn"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("pause_autonomous_task", {
      taskId: taskActive.autonomous_task_id
    });
  });

  it("calls resume and refreshes tasks", async () => {
    mockedInvoke
      .mockResolvedValueOnce([taskPaused])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([{ ...taskPaused, state: "active" }]);

    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="autonomous-resume-btn"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("resume_autonomous_task", {
      taskId: taskPaused.autonomous_task_id
    });
  });

  it("calls cancel and refreshes tasks", async () => {
    mockedInvoke
      .mockResolvedValueOnce([taskActive])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([]);

    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="autonomous-cancel-btn"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("cancel_autonomous_task", {
      taskId: taskActive.autonomous_task_id,
      sessionId: taskActive.current_session_id
    });
  });

  it("shows error alert when fetch fails", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("Network error"));
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="autonomous-error"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="autonomous-error"]').text()).toContain("Network error");
  });

  it("truncates long goals in card view", async () => {
    const longGoal = "A".repeat(120);
    mockedInvoke.mockResolvedValueOnce([{ ...taskActive, goal: longGoal }]);
    const wrapper = mountPane();
    await flushPromises();

    const goalElement = wrapper.find(".autonomous-pane__goal");
    expect(goalElement.text().length).toBeLessThan(120);
    expect(goalElement.text()).toContain("…");
  });

  it("shows full goal in detail panel", async () => {
    const longGoal = "A".repeat(120);
    mockedInvoke
      .mockResolvedValueOnce([{ ...taskActive, goal: longGoal }])
      .mockResolvedValueOnce([]);

    const wrapper = mountPane();
    await flushPromises();

    await wrapper
      .find(`[data-test="autonomous-task-${taskActive.autonomous_task_id}"]`)
      .trigger("click");
    await flushPromises();

    const detailTitle = wrapper.find(".autonomous-pane__detail-title");
    expect(detailTitle.text()).toBe(longGoal);
  });
});
