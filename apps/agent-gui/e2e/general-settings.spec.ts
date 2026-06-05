import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

async function navigateToGeneral(page: Page) {
  await page.goto("/#/settings/general");
  await expect(page.locator(".general-settings")).toBeVisible();
}

test.describe("General Settings", () => {
  test("navigates to general settings via settings tab", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-test="nav-settings"]', { timeout: 10000 });
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-general").click();
    await expect(page.locator(".general-settings")).toBeVisible();
  });

  test("renders locale selector with three options", async ({ page }) => {
    await navigateToGeneral(page);

    const localeSelect = page.getByTestId("settings-locale");
    await expect(localeSelect).toBeVisible();

    const options = localeSelect.locator("option");
    await expect(options).toHaveCount(3);
  });

  test("renders theme selector with three options", async ({ page }) => {
    await navigateToGeneral(page);

    const themeSelect = page.getByTestId("settings-theme");
    await expect(themeSelect).toBeVisible();

    const options = themeSelect.locator("option");
    await expect(options).toHaveCount(3);
  });

  test("theme selector is inside a toggle row", async ({ page }) => {
    await navigateToGeneral(page);

    const themeRow = page.getByTestId("theme-toggle");
    await expect(themeRow).toBeVisible();
    await expect(themeRow.getByTestId("settings-theme")).toBeVisible();
  });

  test("renders devtools toggle in advanced section", async ({ page }) => {
    await navigateToGeneral(page);

    const devtoolsRow = page.getByTestId("settings-devtools-row");
    await expect(devtoolsRow).toBeVisible();

    const devtoolsCheckbox = page.getByTestId("settings-devtools");
    await expect(devtoolsCheckbox).toBeVisible();
  });

  test("devtools toggle starts unchecked and can be toggled", async ({ page }) => {
    await navigateToGeneral(page);

    const devtoolsCheckbox = page.getByTestId("settings-devtools");
    await expect(devtoolsCheckbox).not.toBeChecked();

    await devtoolsCheckbox.check({ force: true });
    await expect(devtoolsCheckbox).toBeChecked();

    // After toggling, the restart-required badge should appear
    await expect(page.getByTestId("settings-devtools-restart")).toBeVisible();
  });

  test("renders software update section with version badge", async ({ page }) => {
    await navigateToGeneral(page);

    const versionRow = page.getByTestId("settings-update-version");
    await expect(versionRow).toBeVisible();

    const versionBadge = page.getByTestId("settings-current-version");
    await expect(versionBadge).toBeVisible();
    await expect(versionBadge).toContainText("0.37.0");
  });

  test("renders auto-check toggle defaulting to checked", async ({ page }) => {
    await navigateToGeneral(page);

    const autoCheckRow = page.getByTestId("settings-update-auto-check");
    await expect(autoCheckRow).toBeVisible();

    const autoCheckbox = page.getByTestId("settings-auto-check");
    await expect(autoCheckbox).toBeChecked();
  });

  test("renders check interval selector when auto-check is enabled", async ({ page }) => {
    await navigateToGeneral(page);

    const intervalRow = page.getByTestId("settings-update-interval");
    await expect(intervalRow).toBeVisible();

    const intervalSelect = page.getByTestId("settings-check-interval");
    await expect(intervalSelect).toBeVisible();

    const options = intervalSelect.locator("option");
    await expect(options).toHaveCount(5);
  });

  test("hides check interval selector when auto-check is disabled", async ({ page }) => {
    await navigateToGeneral(page);

    const autoCheckbox = page.getByTestId("settings-auto-check");
    await expect(autoCheckbox).toBeChecked();

    // Disable auto-check
    await autoCheckbox.uncheck({ force: true });
    await expect(autoCheckbox).not.toBeChecked();

    // Interval row should disappear
    await expect(page.getByTestId("settings-update-interval")).toHaveCount(0);
  });

  test("renders auto-download toggle defaulting to unchecked", async ({ page }) => {
    await navigateToGeneral(page);

    const autoDownloadRow = page.getByTestId("settings-update-auto-download");
    await expect(autoDownloadRow).toBeVisible();

    const autoDownloadCheckbox = page.getByTestId("settings-auto-download");
    await expect(autoDownloadCheckbox).not.toBeChecked();
  });

  test("renders check-now button in update actions", async ({ page }) => {
    await navigateToGeneral(page);

    const actionsRow = page.getByTestId("settings-update-actions");
    await expect(actionsRow).toBeVisible();

    const checkNowBtn = page.getByTestId("settings-check-update");
    await expect(checkNowBtn).toBeVisible();
  });
});
