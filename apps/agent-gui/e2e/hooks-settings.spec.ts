import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("Hooks Settings", () => {
  async function navigateToHooks(page: Page) {
    await page.goto("/#/settings/hooks");
    await expect(page.getByTestId("hooks-settings-pane")).toBeVisible();
  }

  test("creates, edits, and deletes a hook", async ({ page }) => {
    await navigateToHooks(page);

    await expect(page.getByTestId("hook-template-stop-validation")).toBeVisible();
    await page.getByTestId("hook-template-stop-validation").click();
    await expect(page.getByTestId("hook-editor-dialog")).toBeVisible();
    await expect(page.getByTestId("hook-id")).toHaveValue("stop-validation");
    await expect(page.getByTestId("hook-command")).toHaveValue(
      "cargo test --workspace --all-targets"
    );

    await page.getByTestId("hook-save").click();
    await expect(page.getByTestId("hook-row-stop-validation")).toBeVisible();
    await expect(page.getByTestId("hook-row-stop-validation")).toContainText(
      "cargo test --workspace --all-targets"
    );

    await page.getByTestId("hook-edit-stop-validation").click();
    await page.getByTestId("hook-command").fill("cargo test -p agent-runtime");
    await page.getByTestId("hook-save").click();
    await expect(page.getByTestId("hook-row-stop-validation")).toContainText(
      "cargo test -p agent-runtime"
    );

    await page.getByTestId("hook-delete-stop-validation").click();
    await expect(page.getByTestId("hook-row-stop-validation")).toHaveCount(0);
    await expect(page.getByTestId("hooks-empty")).toBeVisible();
  });

  test("filters hooks by search", async ({ page }) => {
    await navigateToHooks(page);

    await page.getByTestId("hook-template-stop-validation").click();
    await expect(page.getByTestId("hook-editor-dialog")).toBeVisible();
    await page.getByTestId("hook-save").click();

    await page.getByTestId("hook-template-prompt-secret-scan").click();
    await expect(page.getByTestId("hook-id")).toHaveValue("prompt-secret-scan");
    await page.getByTestId("hook-save").click();

    await expect(page.getByTestId("hook-row-stop-validation")).toBeVisible();
    await expect(page.getByTestId("hook-row-prompt-secret-scan")).toBeVisible();

    const search = page.getByTestId("hook-search-input");
    await expect(search).toBeVisible();

    await search.fill("secret");
    await expect(page.getByTestId("hook-row-prompt-secret-scan")).toBeVisible();
    await expect(page.getByTestId("hook-row-stop-validation")).toHaveCount(0);

    await search.fill("cargo test");
    await expect(page.getByTestId("hook-row-stop-validation")).toBeVisible();
    await expect(page.getByTestId("hook-row-prompt-secret-scan")).toHaveCount(0);

    await search.fill("does-not-exist");
    await expect(page.getByTestId("hooks-filter-empty")).toContainText(
      "No hooks match your search."
    );
    await expect(page.getByTestId("hooks-list")).toHaveCount(0);

    await search.clear();
    await expect(page.getByTestId("hook-row-stop-validation")).toBeVisible();
    await expect(page.getByTestId("hook-row-prompt-secret-scan")).toBeVisible();
  });

  test("saves project hooks independently from user hooks", async ({ page }) => {
    await navigateToHooks(page);

    await page.getByTestId("source-btn-project").click();
    await page.getByTestId("hook-add").click();
    await expect(page.getByTestId("hook-editor-dialog")).toBeVisible();
    await page.getByTestId("hook-id").fill("project-stop");
    await page.getByTestId("hook-event").selectOption("Stop");
    await page.getByTestId("hook-matcher").fill("*");
    await page.getByTestId("hook-command").fill("bun run lint");
    await page.getByTestId("hook-save").click();

    await expect(page.getByTestId("hook-row-project-stop")).toBeVisible();

    await page.getByTestId("source-btn-user").click();
    await expect(page.getByTestId("hook-row-project-stop")).toHaveCount(0);
    await expect(page.getByTestId("hooks-empty")).toBeVisible();

    await page.getByTestId("source-btn-project").click();
    await expect(page.getByTestId("hook-row-project-stop")).toBeVisible();
  });
});
