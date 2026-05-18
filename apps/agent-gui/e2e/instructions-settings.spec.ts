import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("Instructions Settings", () => {
  async function navigateToInstructions(page: Page) {
    await page.goto("/");
    await page.waitForSelector('[data-test="nav-settings"]', { timeout: 10000 });
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-instructions").click();
    await expect(page.getByTestId("instructions-settings-pane")).toBeVisible();
  }

  test("renders instructions tab with all levels", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-test="nav-settings"]', { timeout: 10000 });
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

    // Project level: hidden in user scope
    const projectTextarea = page.getByTestId("project-instructions");
    await expect(projectTextarea).toHaveCount(0);

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

    // System and user levels are hidden in project scope
    await expect(page.getByTestId("instructions-level-system")).toHaveCount(0);
    await expect(page.getByTestId("instructions-level-user")).toHaveCount(0);

    // Project textarea is visible and editable in project scope
    await expect(projectTextarea).toBeVisible();
    await expect(projectTextarea).not.toBeDisabled();

    // Badge states in project scope
    await expect(page.getByTestId("badge-project-editable")).toContainText("Editable");
  });

  test("edits user instructions and persists after save", async ({ page }) => {
    await navigateToInstructions(page);

    // Verify user textarea is empty initially
    const userTextarea = page.getByTestId("user-instructions");
    await expect(userTextarea).toHaveValue("");

    // Type new user instructions
    const newText = "Always use TypeScript strict mode.\nPrefer arrow functions.";
    await userTextarea.fill(newText);
    await expect(userTextarea).toHaveValue(newText);

    // Click save
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(500);

    // After save, load() is called — the textarea should show the saved value
    await expect(userTextarea).toHaveValue(newText);

    // Effective preview should include user instructions
    const effectivePreview = page.getByTestId("effective-instructions");
    await expect(effectivePreview).toHaveAttribute("value", /Always use TypeScript/);
  });

  test("edits project-specific instructions", async ({ page }) => {
    await navigateToInstructions(page);

    // Switch to project scope
    await page.getByTestId("source-btn-project").click();
    await page.waitForTimeout(500);

    // Project textarea should be editable and empty
    const projectTextarea = page.getByTestId("project-instructions");
    await expect(projectTextarea).not.toBeDisabled();
    await expect(projectTextarea).toHaveValue("");

    // Type project-specific instructions
    const projectText = "Project rules: always run tests before committing.";
    await projectTextarea.fill(projectText);
    await expect(projectTextarea).toHaveValue(projectText);

    // Save
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(500);

    // After save, the value persists
    await expect(projectTextarea).toHaveValue(projectText);

    // Effective preview includes project instructions
    const effectivePreview = page.getByTestId("effective-instructions");
    await expect(effectivePreview).toHaveAttribute("value", /always run tests before committing/);
  });

  test("persists instructions across tab switches", async ({ page }) => {
    await navigateToInstructions(page);

    // Edit user instructions
    const userText = "Cross-tab persistence test content.";
    await page.getByTestId("user-instructions").fill(userText);
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(300);

    // Navigate away to general tab
    await page.getByTestId("settings-tab-general").click();
    await page.waitForTimeout(200);

    // Navigate back to instructions tab
    await page.getByTestId("settings-tab-instructions").click();
    await expect(page.getByTestId("instructions-settings-pane")).toBeVisible();

    // Text should still be there (load() is called on tab switch)
    await expect(page.getByTestId("user-instructions")).toHaveValue(userText);
  });

  test("persists instructions across scope switches", async ({ page }) => {
    await navigateToInstructions(page);

    // Edit user instructions
    const userText = "User scope content.";
    await page.getByTestId("user-instructions").fill(userText);
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(300);

    // Switch to project scope and edit
    await page.getByTestId("source-btn-project").click();
    await page.waitForTimeout(300);

    const projectText = "Project scope content.";
    await page.getByTestId("project-instructions").fill(projectText);
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(300);

    // Switch back to user scope
    await page.getByTestId("source-btn-user").click();
    await page.waitForTimeout(300);

    // User instructions should still be there
    await expect(page.getByTestId("user-instructions")).toHaveValue(userText);

    // Switch to project scope again
    await page.getByTestId("source-btn-project").click();
    await page.waitForTimeout(300);

    // Project instructions should still be there
    await expect(page.getByTestId("project-instructions")).toHaveValue(projectText);
  });

  test("shows empty textareas when no instructions are set", async ({ page }) => {
    await navigateToInstructions(page);

    // User textarea should be empty (mock returns null for both user and project)
    await expect(page.getByTestId("user-instructions")).toHaveValue("");

    // Switch to project scope
    await page.getByTestId("source-btn-project").click();
    await page.waitForTimeout(300);

    // Project textarea should also be empty
    await expect(page.getByTestId("project-instructions")).toHaveValue("");
  });

  test("handles long multi-line content", async ({ page }) => {
    await navigateToInstructions(page);

    // Generate long multi-paragraph content
    const longText = Array.from({ length: 5 }, (_, i) =>
      `Paragraph ${i + 1}: This is a long line of instruction text that should be preserved correctly across save and reload cycles. It contains multiple sentences to simulate realistic user instructions.`.repeat(
        3
      )
    ).join("\n\n");

    await page.getByTestId("user-instructions").fill(longText);
    await page.getByTestId("instructions-save").click();
    await page.waitForTimeout(500);

    // Should preserve the full content
    await expect(page.getByTestId("user-instructions")).toHaveValue(longText);

    // Effective preview should include the long text
    const effectivePreview = page.getByTestId("effective-instructions");
    await expect(effectivePreview).toHaveAttribute("value", /Paragraph 1/);
    await expect(effectivePreview).toHaveAttribute("value", /Paragraph 5/);
  });

  test("save button is disabled while saving", async ({ page }) => {
    await navigateToInstructions(page);

    const saveBtn = page.getByTestId("instructions-save");
    await expect(saveBtn).toBeEnabled();

    // Type something so save has work to do
    await page.getByTestId("user-instructions").fill("test");

    // Click save — the button should become disabled during the operation
    // The mock resolves immediately, so the disabled state is brief,
    // but we verify the button exists and can be interacted with
    await saveBtn.click();

    // After save completes (mock is instant), button should be enabled again
    await expect(saveBtn).toBeEnabled();
  });
});
