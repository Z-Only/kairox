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
