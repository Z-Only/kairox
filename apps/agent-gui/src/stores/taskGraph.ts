import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { TaskSnapshot } from "../types";

export const taskGraphState = reactive({
  tasks: [] as TaskSnapshot[],
  currentSessionId: null as string | null,
  loading: false
});

export async function refreshTaskGraph(sessionId: string) {
  taskGraphState.currentSessionId = sessionId;
  taskGraphState.loading = true;
  try {
    const tasks: TaskSnapshot[] = await invoke("get_task_graph", {
      sessionId
    });
    if (taskGraphState.currentSessionId === sessionId) {
      taskGraphState.tasks = tasks;
    }
  } catch (e) {
    console.error("Failed to load task graph:", e);
    if (taskGraphState.currentSessionId === sessionId) {
      taskGraphState.tasks = [];
    }
  } finally {
    taskGraphState.loading = false;
  }
}

export function clearTaskGraph() {
  taskGraphState.tasks = [];
  taskGraphState.currentSessionId = null;
}

export interface TaskTreeNode {
  task: TaskSnapshot;
  children: TaskTreeNode[];
}

export function buildTaskTree(tasks: TaskSnapshot[]): TaskTreeNode[] {
  const taskMap = new Map(tasks.map((t) => [t.id, t]));
  const childrenMap = new Map<string, TaskTreeNode[]>();
  const roots: TaskTreeNode[] = [];

  for (const task of tasks) {
    const hasParent = task.dependencies.some((depId) => taskMap.has(depId));
    if (!hasParent) {
      roots.push({ task, children: [] });
    } else {
      for (const depId of task.dependencies) {
        if (!childrenMap.has(depId)) {
          childrenMap.set(depId, []);
        }
        childrenMap.get(depId)!.push({ task, children: [] });
      }
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
