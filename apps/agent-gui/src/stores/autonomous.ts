import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { AutonomousTaskView, CheckpointView } from "@/types";

export const useAutonomousStore = defineStore("autonomous", () => {
  const tasks = ref<AutonomousTaskView[]>([]);
  const selectedTaskId = ref<string | null>(null);
  const checkpoints = ref<CheckpointView[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const selectedTask = computed(
    () => tasks.value.find((t) => t.autonomous_task_id === selectedTaskId.value) ?? null
  );

  const activeTasks = computed(() => tasks.value.filter((t) => t.state === "active"));

  async function fetchTasks() {
    loading.value = true;
    error.value = null;
    try {
      tasks.value = await invoke<AutonomousTaskView[]>("list_autonomous_tasks");
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function fetchTask(taskId: string) {
    try {
      const task = await invoke<AutonomousTaskView | null>("get_autonomous_task", { taskId });
      selectedTaskId.value = task ? task.autonomous_task_id : null;
      if (task) {
        const index = tasks.value.findIndex((t) => t.autonomous_task_id === taskId);
        if (index >= 0) {
          tasks.value[index] = task;
        }
      }
    } catch (e) {
      error.value = String(e);
    }
  }

  async function fetchCheckpoints(taskId: string) {
    loading.value = true;
    error.value = null;
    try {
      checkpoints.value = await invoke<CheckpointView[]>("get_autonomous_checkpoints", { taskId });
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function pauseTask(taskId: string) {
    await invoke("pause_autonomous_task", { taskId });
    await fetchTasks();
  }

  async function resumeTask(taskId: string) {
    await invoke("resume_autonomous_task", { taskId });
    await fetchTasks();
  }

  async function cancelTask(taskId: string, sessionId: string) {
    await invoke("cancel_autonomous_task", { taskId, sessionId });
    await fetchTasks();
  }

  function selectTask(taskId: string | null) {
    selectedTaskId.value = taskId;
  }

  function $reset() {
    tasks.value = [];
    selectedTaskId.value = null;
    checkpoints.value = [];
    loading.value = false;
    error.value = null;
  }

  return {
    tasks,
    selectedTaskId,
    checkpoints,
    loading,
    error,
    selectedTask,
    activeTasks,
    fetchTasks,
    fetchTask,
    fetchCheckpoints,
    pauseTask,
    resumeTask,
    cancelTask,
    selectTask,
    $reset
  };
});
