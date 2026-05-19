/**
 * E2E: Multi-agent flow — planner decomposition, parallel workers, blocked tasks.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test("planner decomposes task into sub-tasks", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message that triggers planner decomposition.
  await page.locator('textarea[data-test="message-input"]').fill("/plan Build a web server");
  await page.getByTestId("send-button").click();

  // Wait for the mock response
  await expect(page.locator(".message").first()).toBeVisible({
    timeout: 5_000
  });
});

test("parallel workers appear with distinct badges", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  // Simulate a planner creating two parallel worker tasks
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const _planId = mock.simulateTaskCreated("Plan: Feature X", "Planner");
    const worker1Id = mock.simulateTaskCreated("Implement auth", "Worker");
    const worker2Id = mock.simulateTaskCreated("Implement API", "Worker");
    mock.simulateAgentSpawned("agent_w1", "Worker", worker1Id);
    mock.simulateAgentSpawned("agent_w2", "Worker", worker2Id);
  });

  // Both worker role badges should be visible
  const workerBadges = page.locator(".task-role");
  await expect(workerBadges).toHaveCount(3, { timeout: 3_000 }); // 1 Planner + 2 Workers
});

test("agent spawned and idle lifecycle", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  // Create a task, spawn an agent, start, complete, then idle
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const taskId = mock.simulateTaskCreated("Write tests", "Worker");
    mock.simulateAgentSpawned("agent_w1", "Worker", taskId);
    mock.simulateTaskTransition(taskId, "AgentTaskStarted");
    mock.simulateTaskTransition(taskId, "AgentTaskCompleted");
    mock.simulateAgentIdle("agent_w1");
  });

  // Task should be completed
  await expect(page.locator(".task-status").first()).toContainText("✅", {
    timeout: 3_000
  });
});

test("task retry button appears for failed tasks", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  // Create and fail a task
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const taskId = mock.simulateTaskCreated("Flaky task", "Worker");
    mock.simulateTaskTransition(taskId, "AgentTaskStarted");
    mock.simulateTaskTransition(taskId, "AgentTaskFailed", "Timeout");
  });

  // Should show retry button for failed task
  await expect(page.getByTestId("task-retry").first()).toBeVisible({
    timeout: 3_000
  });
});

test("task decomposition event creates system message in chat", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate task decomposition
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const parentId = mock.simulateTaskCreated("Plan", "Planner");
    const sub1 = mock.simulateTaskCreated("Sub 1", "Worker");
    const sub2 = mock.simulateTaskCreated("Sub 2", "Worker");
    mock.simulateTaskDecomposed(parentId, [sub1, sub2]);
  });

  // A system message about decomposition should appear in chat
  await expect(page.locator(".message-system").first()).toBeVisible({
    timeout: 3_000
  });
  await expect(page.locator(".message-system").first()).toContainText("decomposed");
});
