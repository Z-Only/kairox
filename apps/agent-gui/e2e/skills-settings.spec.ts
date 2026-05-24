import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/#/settings/skills");
  await expect(page.getByTestId("skill-settings-pane")).toBeVisible();
});

test.describe("Skills Settings", () => {
  test("filters installed skills by search", async ({ page }) => {
    await page.getByTestId("source-btn-project").click();

    await expect(page.getByTestId("skill-installed-search-input")).toBeVisible();

    await page.getByTestId("skill-installed-search-input").fill("registry");
    await expect(page.getByTestId("skill-row-project-registry-review")).toBeVisible();
    await expect(page.getByTestId("skill-row-project-project-review")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("check failed");
    await expect(page.getByTestId("skill-row-project-invalid-workspace-skill")).toBeVisible();
    await expect(page.getByTestId("skill-row-project-registry-review")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("does-not-exist");
    await expect(page.getByTestId("skill-installed-filter-empty")).toContainText(
      "No installed skills match your search."
    );
    await expect(page.getByTestId("skill-installed-list")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("");
    await expect(page.getByTestId("skill-row-project-project-review")).toBeVisible();
    await expect(page.getByTestId("skill-row-user-user-planning")).toBeVisible();
  });
});
