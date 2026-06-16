import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

async function openWorkbench(page: Page) {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
}

async function dragHorizontally(page: Page, selector: string, deltaX: number) {
  const box = await page.locator(selector).boundingBox();
  expect(box).not.toBeNull();
  const startX = box!.x + box!.width / 2;
  const startY = box!.y + box!.height / 2;

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + deltaX, startY, { steps: 5 });
  await page.mouse.up();
}

async function seedSubagents(page: Page) {
  return page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;

    const runningTaskId = mock.simulateTaskCreated("Implement sidebar regression", "Worker");
    mock.simulateAgentSpawned("agent_worker_1", "Worker", runningTaskId);
    mock.simulateTaskTransition(runningTaskId, "AgentTaskStarted");

    const failedTaskId = mock.simulateTaskCreated("Review sidebar regression", "Reviewer");
    mock.simulateAgentSpawned("agent_reviewer_1", "Reviewer", failedTaskId);
    mock.simulateTaskTransition(failedTaskId, "AgentTaskStarted");
    mock.simulateTaskTransition(failedTaskId, "AgentTaskFailed", "Snapshot mismatch");

    const blockedTaskId = mock.simulateTaskCreated("Apply review follow-up", "Worker", [
      failedTaskId
    ]);
    mock.simulateAgentSpawned("agent_worker_blocked", "Worker", blockedTaskId);
    mock.simulateTaskBlocked(blockedTaskId, failedTaskId, "Review task failed");

    return { runningTaskId, failedTaskId, blockedTaskId };
  });
}

function commandCallCount(page: Page, command: string) {
  return page.evaluate((cmd) => {
    const mock = (window as any).__KAIROX_MOCK__;
    return mock.commandCalls(cmd).length;
  }, command);
}

test("collapses and resizes both workbench sidebars with persisted widths", async ({ page }) => {
  await openWorkbench(page);
  await page.evaluate(() => window.localStorage.clear());
  await page.reload();
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  await page.getByTestId("left-sidebar-toggle").click();
  await expect(page.getByTestId("view-workbench")).toHaveClass(/workbench--left-collapsed/);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.left-sidebar-collapsed")))
    .toBe("true");

  await page.getByTestId("left-sidebar-toggle").click();
  await expect(page.getByTestId("view-workbench")).not.toHaveClass(/workbench--left-collapsed/);

  await dragHorizontally(page, '[data-test="left-sidebar-resizer"]', 50);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.left-sidebar-width")))
    .toBe("270");

  await dragHorizontally(page, '[data-test="right-sidebar-resizer"]', -40);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.right-sidebar-width")))
    .toBe("320");

  await page.reload();
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect
    .poll(() => page.locator(".left-sidebar").boundingBox())
    .toMatchObject({
      width: 270
    });
  await expect
    .poll(() => page.locator(".right-sidebar").boundingBox())
    .toMatchObject({
      width: 320
    });
});

test("keeps project and regular session navigation in separate scroll panes", async ({ page }) => {
  await openWorkbench(page);

  await expect(page.getByTestId("projects-scroll-region")).toBeVisible();
  await expect(page.getByTestId("sessions-scroll-region")).toBeVisible();

  await expect
    .poll(() =>
      page.evaluate(() => ({
        outerOverflow: getComputedStyle(document.querySelector(".session-scroll")!).overflowY,
        projectOverflow: getComputedStyle(
          document.querySelector('[data-test="projects-scroll-region"]')!
        ).overflowY,
        sessionsOverflow: getComputedStyle(
          document.querySelector('[data-test="sessions-scroll-region"]')!
        ).overflowY,
        sectionMaxHeights: Array.from(
          document.querySelectorAll(".sessions-sidebar .sidebar-section")
        ).map((section) => getComputedStyle(section).maxHeight)
      }))
    )
    .toEqual({
      outerOverflow: "hidden",
      projectOverflow: "auto",
      sessionsOverflow: "auto",
      sectionMaxHeights: ["50%", "50%"]
    });
});

test("shows subagents with bound tasks and drives attention task actions", async ({ page }) => {
  await openWorkbench(page);
  await seedSubagents(page);

  await page.getByTestId("trace-tab-subagents").click();
  await expect(page.getByTestId("subagent-panel")).toBeVisible();

  const runningWorker = page.getByTestId("subagent-card-agent_worker_1");
  await expect(runningWorker.locator(".subagent-role")).toHaveText("Worker");
  await expect(runningWorker.locator(".subagent-label")).toHaveText(/W:\d+/);
  await expect(runningWorker.locator(".subagent-status")).toContainText("running");
  await expect(runningWorker.locator(".subagent-task-title")).toHaveText(
    "Implement sidebar regression"
  );
  await expect(runningWorker.locator(".subagent-task-state")).toContainText("Running");

  const failedReviewer = page.getByTestId("subagent-card-agent_reviewer_1");
  await expect(failedReviewer.locator(".subagent-role")).toHaveText("Reviewer");
  await expect(failedReviewer.locator(".subagent-label")).toHaveText("R");
  await expect(failedReviewer.locator(".subagent-status")).toContainText("failed");
  await expect(failedReviewer.locator(".subagent-task-title")).toHaveText(
    "Review sidebar regression"
  );
  await expect(failedReviewer.locator(".subagent-task-state")).toContainText("Failed");

  await page.getByTestId("subagent-filter-attention").click();
  await expect(runningWorker).toHaveCount(0);
  await expect(failedReviewer).toBeVisible();
  await expect(page.getByTestId("subagent-card-agent_worker_blocked")).toContainText(
    "Review task failed"
  );

  await page.getByTestId("subagent-filter-all").click();
  await page.getByTestId("subagent-retry-agent_reviewer_1").click();
  await expect.poll(() => commandCallCount(page, "retry_task")).toBe(1);
  await expect(failedReviewer.locator(".subagent-status")).toContainText("running");
  await expect(failedReviewer.locator(".subagent-task-state")).toContainText("Running");

  await page.getByTestId("subagent-cancel-agent_reviewer_1").click();
  await expect.poll(() => commandCallCount(page, "cancel_task")).toBe(1);
  await expect(failedReviewer.locator(".subagent-task-state")).toContainText("Cancelled");
});
