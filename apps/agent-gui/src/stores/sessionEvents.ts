import type { Ref } from "vue";
import type {
  SessionProjection,
  DomainEvent,
  ContextUsage,
  ProjectedModelLimits,
  CompactionStatus
} from "@/types";
import { agentRoleToProjectedRole } from "@/types";
import type { useAgentsStore } from "@/stores/agents";
import type { useTaskGraphStore } from "@/stores/taskGraph";

export function emptyProjection(): SessionProjection {
  return {
    messages: [],
    task_titles: [],
    task_graph: { tasks: [] },
    token_stream: "",
    cancelled: false,
    last_context_usage: null,
    model_limits: null,
    compaction: { type: "Idle" }
  };
}

export interface EventReducerContext {
  projection: Ref<SessionProjection>;
  isStreaming: Ref<boolean>;
  lastSendError: Ref<string | null>;
  lastContextUsage: Ref<ContextUsage | null>;
  compacting: Ref<boolean>;
  lastCompactionError: Ref<string | null>;
  currentProfile: Ref<string>;
  currentReasoningEffort: Ref<string | null>;
  modelLimits: Ref<ProjectedModelLimits | null>;
}

export function applySessionEvent(
  event: DomainEvent,
  ctx: EventReducerContext,
  agentsStore: ReturnType<typeof useAgentsStore>
): void {
  const p = event.payload;
  const sourceAgentId = event.source_agent_id;

  switch (p.type) {
    case "UserMessageAdded": {
      ctx.lastSendError.value = null;
      ctx.projection.value.messages.push({
        role: "user",
        content: p.content
      });
      ctx.isStreaming.value = true;
      break;
    }
    case "ModelTokenDelta": {
      ctx.projection.value.token_stream += p.delta;
      break;
    }
    case "AssistantMessageCompleted": {
      const msg: (typeof ctx.projection.value.messages)[0] = {
        role: "assistant",
        content: p.content
      };
      if (sourceAgentId && sourceAgentId !== "agent_system") {
        msg.sourceAgentId = sourceAgentId;
        const agent = agentsStore.agents.get(sourceAgentId);
        if (agent) {
          msg.role = agentRoleToProjectedRole(agent.role);
        }
      }
      ctx.projection.value.messages.push(msg);
      ctx.projection.value.token_stream = "";
      ctx.isStreaming.value = false;
      break;
    }
    case "SessionCancelled":
      ctx.projection.value.cancelled = true;
      ctx.isStreaming.value = false;
      break;
    case "AgentTaskCreated": {
      ctx.projection.value.task_titles.push(p.title);
      break;
    }
    case "AgentTaskStarted":
      break;
    case "AgentTaskCompleted": {
      ctx.isStreaming.value = false;
      break;
    }
    case "AgentTaskFailed": {
      ctx.projection.value.messages.push({
        role: "assistant",
        content: `[error] ${p.error || "Unknown error"}`
      });
      ctx.projection.value.token_stream = "";
      ctx.isStreaming.value = false;
      break;
    }
    case "TaskDecomposed": {
      ctx.projection.value.messages.push({
        role: "system",
        content: `Task decomposed into ${p.sub_task_ids.length} sub-tasks`
      });
      break;
    }
    case "TaskBlocked": {
      ctx.projection.value.messages.push({
        role: "system",
        content: `Task blocked: ${p.reason || "dependency failed"}`
      });
      break;
    }
    case "TaskRetried": {
      ctx.projection.value.messages.push({
        role: "system",
        content: `Task retry attempt ${p.attempt}`
      });
      break;
    }
    case "ContextAssembled": {
      ctx.lastContextUsage.value = p.usage;
      break;
    }
    case "ContextCompactionStarted": {
      ctx.compacting.value = true;
      ctx.lastCompactionError.value = null;
      break;
    }
    case "ContextCompactionCompleted": {
      ctx.compacting.value = false;
      break;
    }
    case "ContextCompactionFailed": {
      ctx.compacting.value = false;
      ctx.lastCompactionError.value = p.error;
      break;
    }
    case "ContextCompactionSkipped": {
      ctx.projection.value.compaction = {
        type: "Skipped",
        reason: p.reason,
        ratio: p.ratio
      };
      ctx.compacting.value = false;
      ctx.lastCompactionError.value = null;
      break;
    }
    case "ModelProfileSwitched": {
      ctx.currentProfile.value = p.to_profile;
      ctx.currentReasoningEffort.value = p.reasoning_effort ?? null;
      ctx.modelLimits.value = {
        context_window: p.context_window,
        output_limit: p.output_limit,
        source: p.limit_source
      };
      break;
    }
    case "AgentSpawned":
    case "AgentIdle":
      break;
    case "SessionInitialized":
    case "ModelRequestStarted":
    case "ModelToolCallRequested":
    case "ToolInvocationStarted":
    case "ToolInvocationCompleted":
    case "ToolInvocationFailed":
    case "PermissionRequested":
    case "PermissionGranted":
    case "PermissionDenied":
    case "FilePatchProposed":
    case "FilePatchApplied":
    case "MemoryProposed":
    case "MemoryAccepted":
    case "MemoryRejected":
    case "ReviewerFindingAdded":
    case "WorkspaceOpened":
      break;
  }
}

export function setProjectionFromSnapshot(
  next: SessionProjection,
  ctx: EventReducerContext,
  taskGraphStore: ReturnType<typeof useTaskGraphStore>,
  currentSessionId: string | null
): void {
  const status: CompactionStatus = next.compaction ?? { type: "Idle" };
  ctx.projection.value = { ...next, compaction: status };
  ctx.isStreaming.value = false;
  if (next.task_graph?.tasks) {
    taskGraphStore.setTaskGraph(next.task_graph.tasks, currentSessionId);
  }
  ctx.lastContextUsage.value = next.last_context_usage ?? null;
  ctx.modelLimits.value = next.model_limits ?? null;
  ctx.compacting.value = status.type === "Running";
  ctx.lastCompactionError.value = status.type === "Failed" ? status.error : null;
}

export function resetProjectionState(
  ctx: EventReducerContext,
  agentsStore: ReturnType<typeof useAgentsStore>,
  streamsByTask: Ref<Map<string, string>>
): void {
  ctx.projection.value = emptyProjection();
  ctx.lastSendError.value = null;
  ctx.isStreaming.value = false;
  streamsByTask.value.clear();
  agentsStore.clearAgents();
  ctx.lastContextUsage.value = null;
  ctx.modelLimits.value = null;
  ctx.compacting.value = false;
  ctx.lastCompactionError.value = null;
}
