import { reactive } from "vue";
import type { TaskSnapshot } from "../types";

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
