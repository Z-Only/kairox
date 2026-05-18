import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("Agents Settings", () => {
  async function navigateToAgents(page: Page) {
    await page.goto("/");
    await page.waitForSelector('[data-test="nav-settings"]', { timeout: 10000 });
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-agents").click();
    await expect(page.getByTestId("agent-settings-pane")).toBeVisible();
  }

  test("renders agents and edits a user-scoped agent", async ({ page }) => {
    await navigateToAgents(page);

    await expect(page.getByTestId("agent-row-worker")).toContainText("Built-in");
    await expect(page.getByTestId("agent-row-code-reviewer")).toContainText("smart");

    await page.getByTestId("agent-edit-code-reviewer").click();
    await expect(page.getByTestId("agent-form-name")).toHaveValue("code-reviewer");
    await page.getByTestId("agent-form-description").fill("Review diffs before handoff.");
    await page.getByTestId("agent-form-tools").fill("fs.read, search, shell");
    await page.getByTestId("agent-save").click();

    await expect(page.getByTestId("agent-row-code-reviewer")).toContainText(
      "Review diffs before handoff."
    );
    await expect(page.getByTestId("agent-row-code-reviewer")).toContainText("shell");
  });

  test("copies a built-in agent into user scope", async ({ page }) => {
    await navigateToAgents(page);

    await page.getByTestId("agent-copy-worker").click();

    const workerRows = page.locator('[data-test="agent-row-worker"]');
    await expect(workerRows).toHaveCount(2);
    await expect(workerRows.nth(0)).toContainText("Built-in");
    await expect(workerRows.nth(1)).toContainText("User");
  });
});
