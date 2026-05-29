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

  test("keeps installed skill filter bar compact", async ({ page }) => {
    await page.getByTestId("source-btn-project").click();

    const filterBar = page.getByTestId("skill-installed-filters");
    const searchInput = page.getByTestId("skill-installed-search-input");
    const list = page.getByTestId("skill-installed-list");
    await expect(filterBar).toBeVisible();

    const filterBox = await filterBar.boundingBox();
    const inputBox = await searchInput.boundingBox();
    const listBox = await list.boundingBox();

    expect(filterBox).not.toBeNull();
    expect(inputBox).not.toBeNull();
    expect(listBox).not.toBeNull();
    expect(filterBox!.height).toBeLessThan(72);
    expect(inputBox!.y - filterBox!.y).toBeLessThan(12);
    expect(listBox!.y - (filterBox!.y + filterBox!.height)).toBeLessThan(24);
  });

  test("filters skill catalog sources by search", async ({ page }) => {
    await page.getByTestId("skill-subtab-discover").click();
    await page.getByTestId("skill-source-settings-btn").click();

    await expect(page.getByTestId("skill-source-settings-drawer")).toBeVisible();
    await expect(page.getByTestId("skill-source-search-input")).toBeVisible();

    await page.getByTestId("skill-add-source-toggle").click();
    await page.getByTestId("skill-src-id").fill("custom-skillhub");
    await page.getByTestId("skill-src-name").fill("Custom SkillHub");
    await page.getByTestId("skill-src-url").fill("https://api.skillhub.example");
    await page.getByTestId("skill-src-save").click();

    await page.getByTestId("skill-source-search-input").fill("palebluedot");
    await expect(page.getByTestId("skill-source-row-skillhub")).toBeVisible();
    await expect(page.getByTestId("skill-source-row-custom-skillhub")).toHaveCount(0);

    await page.getByTestId("skill-source-search-input").fill("custom");
    await expect(page.getByTestId("skill-source-row-custom-skillhub")).toBeVisible();
    await expect(page.getByTestId("skill-source-row-skillhub")).toHaveCount(0);

    await page.getByTestId("skill-source-search-input").fill("does-not-exist");
    await expect(page.getByTestId("skill-sources-filter-empty")).toContainText(
      "No skill catalog sources match your search."
    );
    await expect(page.getByTestId("skill-sources-list")).toHaveCount(0);
  });
});
