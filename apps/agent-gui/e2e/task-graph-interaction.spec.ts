/**
 * E2E: Task graph interaction — N-level tree, retry, cancel, agent badges.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

// Selector notes after Task 7 NaiveUI migration: the TraceTimeline tab strip
// renders native buttons inside `.tab-group`, so `.tab-group button` still
// selects the Trace/Tasks/Memory toggles. TaskNode preserves stable hooks
// (`.task-node`, `.task-status`, `.task-role`, `[data-test="task-retry"]`).

test("retrying a failed task clears the error and advances retry attempts", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  const taskId = await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const id = mock.simulateTaskCreated("Retry API request", "Worker");
    mock.simulateTaskTransition(id, "AgentTaskStarted");
    mock.simulateTaskTransition(id, "AgentTaskFailed", "Request timed out");
    return id;
  });

  const task = page.locator(".task-node").first();
  await expect(task.locator(".task-status")).toContainText("❌", {
    timeout: 3_000
  });
  await expect(page.locator(".task-error-text").first()).toContainText("Request timed out");

  await page.getByTestId("task-retry").first().click();

  await expect(task.locator(".task-status")).toContainText("🔄", {
    timeout: 3_000
  });
  await expect(page.locator(".task-error-text")).toHaveCount(0);
  await expect(task.locator(".task-retry")).toContainText("↻1/3");

  await page.evaluate((id) => {
    const mock = (window as any).__KAIROX_MOCK__;
    mock.simulateTaskTransition(id, "AgentTaskFailed", "Still timing out");
  }, taskId);
  await page.getByTestId("task-retry").first().click();

  await expect(task.locator(".task-status")).toContainText("🔄", {
    timeout: 3_000
  });
  await expect(task.locator(".task-retry")).toContainText("↻2/3");
});

test("N-level task tree shows parent-child relationships", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const parentId = mock.simulateTaskCreated("Plan work", "Planner");
    const child1Id = mock.simulateTaskCreated("Implement A", "Worker", [parentId]);
    const child2Id = mock.simulateTaskCreated("Implement B", "Worker", [parentId]);
    mock.simulateTaskCreated("Review A", "Reviewer", [child1Id]);
    mock.simulateTaskDecomposed(parentId, [child1Id, child2Id]);
  });

  const nodes = page.locator(".task-node");
  await expect(nodes).toHaveCount(3, {
    timeout: 3_000
  });
  await expect(nodes.nth(0)).toContainText("Plan work");
  await expect(nodes.nth(1)).toContainText("Implement A");
  await expect(nodes.nth(2)).toContainText("Implement B");

  await nodes.nth(1).click();

  await expect(nodes).toHaveCount(4);
  await expect(nodes.nth(2)).toContainText("Review A");
  await expect(nodes.nth(3)).toContainText("Implement B");
});

test("blocked task shows blocked state", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  // Create tasks and block one
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const parentId = mock.simulateTaskCreated("Parent task", "Planner");
    const childId = mock.simulateTaskCreated("Dependent task", "Worker");
    // Fail the parent
    mock.simulateTaskTransition(parentId, "AgentTaskStarted");
    mock.simulateTaskTransition(parentId, "AgentTaskFailed", "Timeout");
    // Block the child
    mock.simulateTaskBlocked(childId, parentId, "Dependency failed");
  });

  await expect(page.locator(".task-steps")).toBeVisible({ timeout: 3_000 });
  // Should show Blocked status (⏸️)
  await expect(page.locator(".task-status").last()).toContainText("⏸️", {
    timeout: 3_000
  });
});

test("agent badge shows on task with assigned agent", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group button", { hasText: "Tasks" }).click();

  // Create task and spawn an agent for it
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const taskId = mock.simulateTaskCreated("Research task", "Planner");
    mock.simulateAgentSpawned("agent_p1", "Planner", taskId);
  });

  await expect(page.locator(".task-role").first()).toContainText("P", {
    timeout: 3_000
  });
});
