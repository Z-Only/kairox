import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useAutonomousStore } from "./autonomous";
import type { AutonomousTaskView } from "@/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

function makeTask(overrides: Partial<AutonomousTaskView> = {}): AutonomousTaskView {
  return {
    autonomous_task_id: "atk_001",
    workspace_id: "wrk_test",
    goal: "Implement feature X",
    state: "active",
    current_session_id: "ses_001",
    session_count: 1,
    max_sessions: 5,
    created_at: "2026-06-01T00:00:00Z",
    updated_at: "2026-06-01T00:00:00Z",
    ...overrides
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("useAutonomousStore", () => {
  describe("fetchTasks", () => {
    it("calls invoke with list_autonomous_tasks and populates tasks", async () => {
      const taskList = [makeTask(), makeTask({ autonomous_task_id: "atk_002", state: "paused" })];
      mockInvoke.mockResolvedValueOnce(taskList);

      const store = useAutonomousStore();
      await store.fetchTasks();

      expect(mockInvoke).toHaveBeenCalledWith("list_autonomous_tasks");
      expect(store.tasks).toEqual(taskList);
      expect(store.loading).toBe(false);
      expect(store.error).toBeNull();
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("Network error"));

      const store = useAutonomousStore();
      await store.fetchTasks();

      expect(store.error).toBe("Error: Network error");
      expect(store.tasks).toEqual([]);
    });
  });

  describe("selectTask", () => {
    it("sets selectedTaskId", () => {
      const store = useAutonomousStore();

      store.selectTask("atk_001");
      expect(store.selectedTaskId).toBe("atk_001");

      store.selectTask(null);
      expect(store.selectedTaskId).toBeNull();
    });
  });

  describe("selectedTask", () => {
    it("returns matching task from tasks list", async () => {
      const task = makeTask({ autonomous_task_id: "atk_match" });
      mockInvoke.mockResolvedValueOnce([task]);

      const store = useAutonomousStore();
      await store.fetchTasks();
      store.selectTask("atk_match");

      expect(store.selectedTask).toEqual(task);
    });

    it("returns null when no match", () => {
      const store = useAutonomousStore();
      store.selectTask("atk_nonexistent");

      expect(store.selectedTask).toBeNull();
    });
  });

  describe("activeTasks", () => {
    it("filters tasks with state active", async () => {
      const tasks = [
        makeTask({ autonomous_task_id: "atk_a", state: "active" }),
        makeTask({ autonomous_task_id: "atk_b", state: "paused" }),
        makeTask({ autonomous_task_id: "atk_c", state: "active" }),
        makeTask({ autonomous_task_id: "atk_d", state: "completed" })
      ];
      mockInvoke.mockResolvedValueOnce(tasks);

      const store = useAutonomousStore();
      await store.fetchTasks();

      expect(store.activeTasks).toHaveLength(2);
      expect(store.activeTasks.map((t) => t.autonomous_task_id)).toEqual(["atk_a", "atk_c"]);
    });
  });

  describe("fetchTask", () => {
    it("calls invoke with get_autonomous_task and selects the task", async () => {
      const task = makeTask({ autonomous_task_id: "atk_single" });
      mockInvoke.mockResolvedValueOnce([task]); // for fetchTasks
      mockInvoke.mockResolvedValueOnce(task); // for fetchTask

      const store = useAutonomousStore();
      await store.fetchTasks();
      await store.fetchTask("atk_single");

      expect(mockInvoke).toHaveBeenCalledWith("get_autonomous_task", { taskId: "atk_single" });
      expect(store.selectedTaskId).toBe("atk_single");
    });

    it("sets selectedTaskId to null when task not found", async () => {
      mockInvoke.mockResolvedValueOnce(null);

      const store = useAutonomousStore();
      await store.fetchTask("atk_missing");

      expect(store.selectedTaskId).toBeNull();
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("Not found"));

      const store = useAutonomousStore();
      await store.fetchTask("atk_bad");

      expect(store.error).toBe("Error: Not found");
    });
  });

  describe("fetchCheckpoints", () => {
    it("calls invoke with get_autonomous_checkpoints", async () => {
      const checkpointList = [
        {
          checkpoint_id: "cp_001",
          session_id: "ses_001",
          session_index: 0,
          completed_items: ["item1"],
          remaining_items: ["item2"],
          git_sha: "abc123",
          end_reason: "completed",
          created_at: "2026-06-01T00:00:00Z"
        }
      ];
      mockInvoke.mockResolvedValueOnce(checkpointList);

      const store = useAutonomousStore();
      await store.fetchCheckpoints("atk_001");

      expect(mockInvoke).toHaveBeenCalledWith("get_autonomous_checkpoints", { taskId: "atk_001" });
      expect(store.checkpoints).toEqual(checkpointList);
    });
  });

  describe("pauseTask", () => {
    it("calls invoke and refreshes tasks", async () => {
      mockInvoke.mockResolvedValue([]);

      const store = useAutonomousStore();
      await store.pauseTask("atk_001");

      expect(mockInvoke).toHaveBeenCalledWith("pause_autonomous_task", { taskId: "atk_001" });
      expect(mockInvoke).toHaveBeenCalledWith("list_autonomous_tasks");
    });
  });

  describe("resumeTask", () => {
    it("calls invoke and refreshes tasks", async () => {
      mockInvoke.mockResolvedValue([]);

      const store = useAutonomousStore();
      await store.resumeTask("atk_001");

      expect(mockInvoke).toHaveBeenCalledWith("resume_autonomous_task", { taskId: "atk_001" });
      expect(mockInvoke).toHaveBeenCalledWith("list_autonomous_tasks");
    });
  });

  describe("cancelTask", () => {
    it("calls invoke with taskId and sessionId, then refreshes", async () => {
      mockInvoke.mockResolvedValue([]);

      const store = useAutonomousStore();
      await store.cancelTask("atk_001", "ses_001");

      expect(mockInvoke).toHaveBeenCalledWith("cancel_autonomous_task", {
        taskId: "atk_001",
        sessionId: "ses_001"
      });
      expect(mockInvoke).toHaveBeenCalledWith("list_autonomous_tasks");
    });
  });

  describe("$reset", () => {
    it("resets all state to initial values", async () => {
      const taskList = [makeTask()];
      mockInvoke.mockResolvedValueOnce(taskList);

      const store = useAutonomousStore();
      await store.fetchTasks();
      store.selectTask("atk_001");

      expect(store.tasks).toHaveLength(1);
      expect(store.selectedTaskId).toBe("atk_001");

      store.$reset();

      expect(store.tasks).toEqual([]);
      expect(store.selectedTaskId).toBeNull();
      expect(store.checkpoints).toEqual([]);
      expect(store.loading).toBe(false);
      expect(store.error).toBeNull();
    });
  });
});
