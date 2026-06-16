import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import SubagentPanel from "./SubagentPanel.vue";
import { useAgentsStore } from "@/stores/agents";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot } from "@/types";
import en from "@/locales/en.json";
import zhCN from "@/locales/zh-CN.json";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function makeTask(id: string, overrides?: Partial<TaskSnapshot>): TaskSnapshot {
  return {
    id,
    title: `Task ${id}`,
    role: "Worker",
    state: "Pending",
    dependencies: [],
    error: null,
    retry_count: 0,
    max_retries: 3,
    assigned_agent_id: null,
    failure_reason: null,
    ...overrides
  };
}

function mountPanel(locale: "en" | "zh-CN" = "en") {
  return mountWithPlugins(SubagentPanel, { reusePinia: true, locale }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("SubagentPanel", () => {
  it("shows a localized empty state when there are no subagents", () => {
    const wrapper = mountPanel();
    expect(wrapper.get('[data-test="subagent-panel"]').text()).toContain(en.subagents.empty);
  });

  it("renders localized empty state in Chinese", () => {
    const wrapper = mountPanel("zh-CN");
    expect(wrapper.get('[data-test="subagent-panel"]').text()).toContain(zhCN.subagents.empty);
  });

  it("renders subagents with role, label, status, and bound task", () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_worker_1",
      role: "Worker",
      task_id: "task_1"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("task_1", {
          title: "Implement sidebar",
          role: "Worker",
          state: "Running",
          assigned_agent_id: "agent_worker_1"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();

    expect(wrapper.get('[data-test="subagent-summary"]').text()).toContain("1");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("Worker");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("W");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("running");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain(
      "Implement sidebar"
    );
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("Running");
  });

  it("filters attention agents with failed or blocked bound tasks", async () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_ok",
      role: "Worker",
      task_id: "ok"
    });
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_failed",
      role: "Reviewer",
      task_id: "failed"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("ok", {
          title: "Healthy task",
          state: "Running",
          assigned_agent_id: "agent_ok"
        }),
        makeTask("failed", {
          title: "Review failure",
          role: "Reviewer",
          state: "Failed",
          error: "Model failed",
          assigned_agent_id: "agent_failed"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();
    await wrapper.get('[data-test="subagent-filter-attention"]').trigger("click");

    expect(wrapper.text()).toContain("Review failure");
    expect(wrapper.text()).not.toContain("Healthy task");
    expect(wrapper.text()).toContain("Model failed");
  });

  it("calls retry and cancel task actions for the bound task", async () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    const retryTask = vi.spyOn(taskGraph, "retryTask").mockResolvedValue(undefined);
    const cancelTask = vi.spyOn(taskGraph, "cancelTask").mockResolvedValue(undefined);
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_failed",
      role: "Worker",
      task_id: "failed"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("failed", {
          title: "Broken worker",
          state: "Failed",
          retry_count: 1,
          max_retries: 3,
          assigned_agent_id: "agent_failed"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();
    await wrapper.get('[data-test="subagent-retry-agent_failed"]').trigger("click");
    await wrapper.get('[data-test="subagent-cancel-agent_failed"]').trigger("click");

    expect(retryTask).toHaveBeenCalledWith("failed");
    expect(cancelTask).toHaveBeenCalledWith("failed");
  });
});
