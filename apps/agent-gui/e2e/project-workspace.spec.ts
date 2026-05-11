import { expect, test } from "@playwright/test";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const currentDirectory = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(currentDirectory, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
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
