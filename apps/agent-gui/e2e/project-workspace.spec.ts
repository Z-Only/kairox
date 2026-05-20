import { expect, test } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test("creates a blank project and sends first project message", async ({ page }) => {
  await page.goto("/");

  await page.getByTestId("project-create-trigger").click();
  await page.getByTestId("project-create-blank").click();
  await expect(page.getByTestId("project-item").filter({ hasText: "New Project" })).toBeVisible();

  await page.getByTestId("project-new-session-btn").first().click();
  await page.getByTestId("message-input").fill("Explain this project");
  await page.getByTestId("send-button").click();

  await expect(
    page.getByTestId("chat-message").filter({ hasText: "Explain this project" })
  ).toBeVisible();
});

test("creates a project worktree session from the active project chat", async ({ page }) => {
  await page.goto("/");

  await page.getByTestId("project-item").first().hover();
  await page.getByTestId("project-new-session-btn").first().click();
  await expect(page.getByTestId("project-worktree-session-trigger")).toBeVisible();

  await page.getByTestId("project-worktree-session-trigger").click();
  await page.getByTestId("project-worktree-branch-input").fill("feat/e2e-worktree");
  await page.getByTestId("project-worktree-branch-confirm").click();

  await expect(page.getByTestId("session-git-meta")).toContainText("worktree");
  await expect(page.getByTestId("session-git-meta")).toContainText("feat/e2e-worktree");
  await expect(
    page.getByTestId("project-session-btn").filter({ hasText: "feat/e2e-worktree" })
  ).toBeVisible();
});
