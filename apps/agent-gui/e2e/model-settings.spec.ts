import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

async function openModelSettings(page: Page) {
  await page.goto("/#/settings/models");
  await expect(page.getByTestId("model-settings-pane")).toBeVisible();
}

function modelRow(page: Page, alias: string) {
  return page.getByTestId(`model-row-${alias}`);
}

test("filters model profiles by search", async ({ page }) => {
  await openModelSettings(page);

  await expect(modelRow(page, "fast")).toBeVisible();
  await expect(modelRow(page, "smart")).toBeVisible();
  await expect(modelRow(page, "claude")).toBeVisible();
  await expect(modelRow(page, "fake")).toBeVisible();

  const search = page.getByTestId("model-search-input");
  await expect(search).toBeVisible();

  await search.fill("anthropic");
  await expect(modelRow(page, "claude")).toBeVisible();
  await expect(modelRow(page, "fast")).toHaveCount(0);
  await expect(modelRow(page, "smart")).toHaveCount(0);
  await expect(modelRow(page, "fake")).toHaveCount(0);

  await search.fill("gpt-4o-mini");
  await expect(modelRow(page, "fast")).toBeVisible();
  await expect(modelRow(page, "smart")).toHaveCount(0);
  await expect(modelRow(page, "claude")).toHaveCount(0);
  await expect(modelRow(page, "fake")).toHaveCount(0);

  await search.fill("does-not-exist");
  await expect(page.getByTestId("model-filter-empty-state")).toContainText(
    "No model profiles match your search."
  );
  await expect(page.getByTestId("model-list")).toHaveCount(0);

  await search.clear();
  await expect(modelRow(page, "fast")).toBeVisible();
  await expect(modelRow(page, "smart")).toBeVisible();
  await expect(modelRow(page, "claude")).toBeVisible();
  await expect(modelRow(page, "fake")).toBeVisible();
});
