import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { TaskSnapshot } from "@/types";
import { useAgentsStore } from "@/stores/agents";

/**
 * Extended task tree node with UI-relevant computed fields.
 * Supports N-level nesting via recursive children.
 */
export interface TaskTreeNode {
  task: TaskSnapshot;
  children: TaskTreeNode[];
  /** Human-readable label for the assigned agent (e.g., "W:1", "P", "R"). */
  agentLabel: string | null;
  /** Duration in ms for completed/running tasks, or null. */
  durationMs: number | null;
}

export const useTaskGraphStore = defineStore("taskGraph", () => {
  const tasks = ref<TaskSnapshot[]>([]);
  const currentSessionId = ref<string | null>(null);
  const loading = ref(false);

  /** Set task graph data directly (e.g., from SessionProjection.task_graph). */
  function setTaskGraph(next: TaskSnapshot[], sessionId: string | null) {
    tasks.value = next;
    currentSessionId.value = sessionId;
  }

  function clearTaskGraph() {
    tasks.value = [];
    currentSessionId.value = null;
  }

  /**
   * Build an N-level recursive task tree from a flat task list.
   * Uses dependency inference: a task is a child of the task(s) it depends on.
   * A task with no fulfilled dependencies is a root.
   * When a task has multiple dependencies, it becomes a child of the
   * last dependency by ID order (most recent parent) to avoid duplication.
   */
  function buildTaskTree(input: TaskSnapshot[]): TaskTreeNode[] {
    const agents = useAgentsStore();
    const taskMap = new Map(input.map((t) => [t.id, t]));
    const childrenMap = new Map<string, TaskTreeNode[]>();
    const roots: TaskTreeNode[] = [];

    for (const task of input) {
      let bestParentId: string | null = null;
      for (const depId of task.dependencies) {
        if (taskMap.has(depId)) {
          bestParentId = depId;
        }
      }

      const node: TaskTreeNode = {
        task,
        children: [],
        agentLabel: task.assigned_agent_id
          ? agents.agentLabel(task.assigned_agent_id)
          : null,
        durationMs: null
      };

      if (bestParentId === null) {
        roots.push(node);
      } else {
        if (!childrenMap.has(bestParentId)) {
          childrenMap.set(bestParentId, []);
        }
        childrenMap.get(bestParentId)!.push(node);
      }
    }

    function attachChildren(node: TaskTreeNode) {
      node.children = childrenMap.get(node.task.id) || [];
      for (const child of node.children) {
        attachChildren(child);
      }
    }

    for (const root of roots) {
      attachChildren(root);
    }

    return roots;
  }

  /**
   * Retry a failed task via the Tauri backend.
   */
  async function retryTask(taskId: string): Promise<void> {
    const sessionId = currentSessionId.value;
    if (!sessionId) return;
    try {
      await invoke("retry_task", { sessionId, taskId });
    } catch (e) {
      console.error("Failed to retry task:", e);
    }
  }

  /**
   * Cancel a task via the Tauri backend.
   */
  async function cancelTask(taskId: string): Promise<void> {
    const sessionId = currentSessionId.value;
    if (!sessionId) return;
    try {
      await invoke("cancel_task", { sessionId, taskId });
    } catch (e) {
      console.error("Failed to cancel task:", e);
    }
  }

  return {
    tasks,
    currentSessionId,
    loading,
    setTaskGraph,
    clearTaskGraph,
    buildTaskTree,
    retryTask,
    cancelTask
  };
});
