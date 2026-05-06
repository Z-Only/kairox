/**
 * E2E: Task graph interaction — N-level tree, retry, cancel, agent badges.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("task steps panel shows empty state initially", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

  await expect(page.locator(".task-steps")).toContainText("No tasks yet");
});

test("task appears when AgentTaskCreated event fires", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

  // Simulate task creation
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateTaskCreated(
      "Analyze codebase",
      "Planner"
    );
  });

  // Task should appear in the task steps panel
  await expect(page.locator(".task-node").first()).toBeVisible({
    timeout: 3_000
  });
  await expect(page.locator(".task-node").first()).toContainText(
    "Analyze codebase"
  );
  // Should show Planner role badge
  await expect(page.locator(".task-role").first()).toContainText("P");
});

test("task transitions through states", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

  // Create a task
  const taskId = await page.evaluate(() => {
    return (window as any).__KAIROX_MOCK__.simulateTaskCreated(
      "Build feature",
      "Worker"
    );
  });

  await expect(page.locator(".task-node").first()).toBeVisible({
    timeout: 3_000
  });

  // Should show Pending status (⏳)
  await expect(page.locator(".task-status").first()).toContainText("⏳");

  // Start the task
  await page.evaluate((tid) => {
    (window as any).__KAIROX_MOCK__.simulateTaskTransition(
      tid,
      "AgentTaskStarted"
    );
  }, taskId);

  // Should show Running status (🔄)
  await expect(page.locator(".task-status").first()).toContainText("🔄", {
    timeout: 3_000
  });

  // Complete the task
  await page.evaluate((tid) => {
    (window as any).__KAIROX_MOCK__.simulateTaskTransition(
      tid,
      "AgentTaskCompleted"
    );
  }, taskId);

  // Should show Completed status (✅)
  await expect(page.locator(".task-status").first()).toContainText("✅", {
    timeout: 3_000
  });
});

test("task shows error when it fails", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

  // Create and fail a task
  const taskId = await page.evaluate(() => {
    return (window as any).__KAIROX_MOCK__.simulateTaskCreated(
      "Risky operation",
      "Worker"
    );
  });

  await expect(page.locator(".task-node").first()).toBeVisible({
    timeout: 3_000
  });

  // Fail the task
  await page.evaluate((tid) => {
    (window as any).__KAIROX_MOCK__.simulateTaskTransition(
      tid,
      "AgentTaskFailed",
      "Model timeout"
    );
  }, taskId);

  // Should show Failed status (❌)
  await expect(page.locator(".task-status").first()).toContainText("❌", {
    timeout: 3_000
  });
});

test("N-level task tree shows parent-child relationships", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

  // Create a parent task and two child tasks with dependencies
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const parentId = mock.simulateTaskCreated("Plan work", "Planner");
    const child1Id = mock.simulateTaskCreated("Implement A", "Worker");
    const child2Id = mock.simulateTaskCreated("Implement B", "Worker");
    // Simulate decomposition
    mock.simulateTaskDecomposed(parentId, [child1Id, child2Id]);
  });

  // Root task should be visible and expandable
  await expect(page.locator(".task-node").first()).toBeVisible({
    timeout: 3_000
  });
  await expect(page.locator(".task-node").first()).toContainText("Plan work");
});

test("blocked task shows blocked state", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

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
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to Tasks tab
  await page.locator(".tab-group >> button", { hasText: "Tasks" }).click();

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
