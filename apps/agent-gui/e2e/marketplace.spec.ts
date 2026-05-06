/**
 * E2E: MCP Marketplace — browse, filter, install (happy + runtime-missing),
 * uninstall flows backed by tauri-mock fixtures.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
  await page.goto("/");
  await page.getByTestId("nav-marketplace").click();
});

test.describe("Marketplace", () => {
  test("browses the catalog and shows the filesystem entry", async ({
    page
  }) => {
    const card = page
      .getByTestId("catalog-card")
      .filter({ hasText: "Filesystem" });
    await expect(card).toBeVisible();
  });

  test("filters by keyword", async ({ page }) => {
    await page.getByTestId("catalog-search").fill("filesystem");
    await expect(page.getByTestId("catalog-card")).toHaveCount(1);
  });

  test("installs the filesystem entry happy path", async ({ page }) => {
    await page
      .getByTestId("catalog-card")
      .filter({ hasText: "Filesystem" })
      .click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toBeVisible();
    await page.getByTestId("install-close").click();
    await page.getByTestId("tab-installed").click();
    await expect(page.getByTestId("uninstall-filesystem")).toBeEnabled();
  });

  test("runtime-missing path shows a hint", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error injected on window for tauri-mock to read
      window.__MARKETPLACE_FORCE_MISSING__ = ["node"];
    });
    await page
      .getByTestId("catalog-card")
      .filter({ hasText: "Filesystem" })
      .click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toContainText(
      "Missing runtimes"
    );
  });

  test("uninstall removes the entry", async ({ page }) => {
    await page
      .getByTestId("catalog-card")
      .filter({ hasText: "Filesystem" })
      .click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await page.getByTestId("install-close").click();
    await page.getByTestId("tab-installed").click();
    await page.getByTestId("uninstall-filesystem").click();
    await expect(page.getByTestId("uninstall-filesystem")).toHaveCount(0);
  });
});
