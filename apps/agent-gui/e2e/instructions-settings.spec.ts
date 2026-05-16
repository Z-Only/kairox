import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("Instructions Settings", () => {
  test("renders instructions tab with all levels", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-instructions").click();

    const pane = page.getByTestId("instructions-settings-pane");
    await expect(pane).toBeVisible();

    // System level: visible, readonly
    const systemTextarea = page.getByTestId("system-instructions");
    await expect(systemTextarea).toBeVisible();
    await expect(systemTextarea).toHaveAttribute("readonly");

    // User level: visible, editable (no readonly attr in user scope)
    const userTextarea = page.getByTestId("user-instructions");
    await expect(userTextarea).toBeVisible();
    await expect(userTextarea).not.toHaveAttribute("readonly");

    // Project level: visible, disabled (user scope)
    const projectBadge = page.getByTestId("badge-project-disabled");
    await expect(projectBadge).toBeVisible();
    const projectTextarea = page.getByTestId("project-instructions");
    await expect(projectTextarea).toBeVisible();
    await expect(projectTextarea).toBeDisabled();

    // Effective preview visible
    const effectivePreview = page.getByTestId("effective-instructions");
    await expect(effectivePreview).toBeVisible();

    // Save button exists
    const saveBtn = page.getByTestId("instructions-save");
    await expect(saveBtn).toBeVisible();

    // Badge states in user scope
    await expect(page.getByTestId("badge-system")).toContainText("Read-only");
    await expect(page.getByTestId("badge-user-editable")).toContainText("Editable");

    // Switch to project scope
    await page.getByTestId("source-btn-project").click();
    await page.waitForTimeout(500);

    // Project textarea now editable (not disabled)
    await expect(projectTextarea).not.toBeDisabled();

    // User textarea now readonly in project scope
    await expect(userTextarea).toHaveAttribute("readonly");

    // Badge states in project scope
    await expect(page.getByTestId("badge-project-editable")).toContainText("Editable");
    await expect(page.getByTestId("badge-user-readonly")).toContainText("Read-only");
  });
});
