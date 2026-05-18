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
    await expect(page.getByTestId("plugin-row-user-github")).toContainText("GitHub");
    await expect(page.getByTestId("plugin-row-user-github")).toContainText("Enabled");

    await page.getByTestId("plugin-enabled-user-github").click();
    await expect(page.getByTestId("plugin-row-user-github")).toContainText("Disabled");
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
});
