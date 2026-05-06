import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { TaskSnapshot } from "../types";
import { agentLabel } from "./agents";

export const taskGraphState = reactive({
  tasks: [] as TaskSnapshot[],
  currentSessionId: null as string | null,
  loading: false
});

/** Set task graph data directly (e.g., from SessionProjection.task_graph). */
export function setTaskGraph(tasks: TaskSnapshot[], sessionId: string | null) {
  taskGraphState.tasks = tasks;
  taskGraphState.currentSessionId = sessionId;
}

export function clearTaskGraph() {
  taskGraphState.tasks = [];
  taskGraphState.currentSessionId = null;
}

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

/**
 * Build an N-level recursive task tree from a flat task list.
 * Uses dependency inference: a task is a child of the task(s) it depends on.
 * A task with no fulfilled dependencies is a root.
 * When a task has multiple dependencies, it becomes a child of the
 * last dependency by ID order (most recent parent) to avoid duplication.
 */
export function buildTaskTree(tasks: TaskSnapshot[]): TaskTreeNode[] {
  const taskMap = new Map(tasks.map((t) => [t.id, t]));
  const childrenMap = new Map<string, TaskTreeNode[]>();
  const roots: TaskTreeNode[] = [];

  for (const task of tasks) {
    // Find the best parent: the last dependency (by list order) that exists
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
        ? agentLabel(task.assigned_agent_id)
        : null,
      durationMs: null // Will be computed after tree is built if timestamps are available
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
export async function retryTask(taskId: string): Promise<void> {
  const sessionId = taskGraphState.currentSessionId;
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
export async function cancelTask(taskId: string): Promise<void> {
  const sessionId = taskGraphState.currentSessionId;
  if (!sessionId) return;
  try {
    await invoke("cancel_task", { sessionId, taskId });
  } catch (e) {
    console.error("Failed to cancel task:", e);
  }
}
