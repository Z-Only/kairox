import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/");
  await page.getByTestId("nav-settings").click();
  await page.getByTestId("settings-tab-plugins").click();
});

test.describe("Plugin Settings", () => {
  test("renders installed plugins and toggles a user plugin", async ({ page }) => {
    await expect(page.getByTestId("plugin-settings-pane")).toBeVisible();
    const githubRow = page.getByTestId("plugin-row-user-github");
    await expect(githubRow).toContainText("GitHub");
    await expect(githubRow).toContainText("User");
    await expect(githubRow).toContainText("Enabled");
    await expect(page.getByTestId("plugin-audit-user-github")).toHaveCount(0);

    await page.getByTestId("plugin-enabled-user-github").click();
    await expect(githubRow).toContainText("Disabled");
    await expect(page.getByTestId("plugin-audit-user-github")).toHaveCount(0);

    await page.getByTestId("plugin-enabled-user-github").click();
    await expect(githubRow).toContainText("Enabled");
  });

  test("deletes a user plugin and removes it from the list", async ({ page }) => {
    await expect(page.getByTestId("plugin-row-user-github")).toBeVisible();

    await page.getByTestId("plugin-delete-user-github").click();

    await expect(page.getByTestId("plugin-row-user-github")).not.toBeVisible();
  });

  test("filters installed plugins by search", async ({ page }) => {
    await expect(page.getByTestId("plugin-installed-search-input")).toBeVisible();

    await page.getByTestId("plugin-installed-search-input").fill("commit commands");
    await expect(page.getByTestId("plugin-row-project-commit-commands")).toBeVisible();
    await expect(page.getByTestId("plugin-row-user-github")).toHaveCount(0);

    await page.getByTestId("plugin-installed-search-input").fill("builtin");
    await expect(page.getByTestId("plugin-row-builtin-browser")).toBeVisible();
    await expect(page.getByTestId("plugin-row-project-commit-commands")).toHaveCount(0);

    await page.getByTestId("plugin-installed-search-input").fill("does-not-exist");
    await expect(page.getByTestId("plugin-installed-filter-empty")).toContainText(
      "No installed plugins match your search."
    );
    await expect(page.getByTestId("plugin-installed-list")).toHaveCount(0);

    await page.getByTestId("plugin-installed-search-input").fill("");
    await expect(page.getByTestId("plugin-row-user-github")).toBeVisible();
    await expect(page.getByTestId("plugin-row-project-commit-commands")).toBeVisible();
  });

  test("discovers and installs a marketplace plugin into the selected project scope", async ({
    page
  }) => {
    await page.getByTestId("source-btn-project").click();
    await page.getByTestId("plugin-subtab-marketplace").click();
    await page.getByTestId("plugin-catalog-search").fill("quality");
    await page.getByTestId("plugin-catalog-refresh").click();

    await expect(page.getByTestId("plugin-catalog-card")).toContainText("quality-review");
    await page.getByTestId("plugin-install-anthropics-claude-code-quality-review").click();

    await page.getByTestId("plugin-subtab-installed").click();
    await expect(page.getByTestId("plugin-row-project-quality-review")).toContainText(
      "Quality Review"
    );
  });

  test("shows configured plugin marketplaces", async ({ page }) => {
    await page.getByTestId("plugin-subtab-marketplace").click();
    await page.getByTestId("plugin-source-settings-toggle").click();
    await expect(page.getByTestId("plugin-source-claude-plugins-official")).toContainText(
      "Claude Plugins Official"
    );
    await expect(page.getByTestId("plugin-source-anthropics-claude-code")).toContainText(
      "Anthropic Claude Code Demo"
    );
  });

  test("toggles a marketplace source enabled state", async ({ page }) => {
    await page.getByTestId("plugin-subtab-marketplace").click();
    await page.getByTestId("plugin-source-settings-toggle").click();

    const sourceRow = page.getByTestId("plugin-source-claude-plugins-official");
    await expect(sourceRow).toContainText("Enabled");

    await page.getByTestId("plugin-source-enabled-claude-plugins-official").click();

    await expect(sourceRow).toContainText("Disabled");

    await page.getByTestId("plugin-source-enabled-claude-plugins-official").click();

    await expect(sourceRow).toContainText("Enabled");
  });

  test("filters catalog entries by marketplace", async ({ page }) => {
    await page.getByTestId("plugin-subtab-marketplace").click();

    await expect(page.getByTestId("plugin-catalog-card")).toHaveCount(2);

    await page.getByTestId("plugin-marketplace-filter").selectOption("claude-plugins-official");
    await page.getByTestId("plugin-catalog-refresh").click();

    await expect(page.getByTestId("plugin-catalog-card")).toHaveCount(1);
    await expect(page.getByTestId("plugin-catalog-card")).toContainText("linear");
  });

  test("searches catalog by keyword", async ({ page }) => {
    await page.getByTestId("plugin-subtab-marketplace").click();

    await page.getByTestId("plugin-catalog-search").fill("linear");
    await page.getByTestId("plugin-catalog-refresh").click();

    await expect(page.getByTestId("plugin-catalog-card")).toHaveCount(1);
    await expect(page.getByTestId("plugin-catalog-card")).toContainText("linear");
  });
});
