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

test.describe("Marketplace — Phase 2 remote catalog sources", () => {
  test("user can add and remove a remote catalog source", async ({ page }) => {
    // Open the source-settings drawer.
    await page.getByTestId("catalog-source-settings").click();
    await expect(
      page.getByTestId("catalog-source-settings-drawer")
    ).toBeVisible();
    await expect(page.getByText("No remote catalog sources")).toBeVisible();

    // Add a new source.
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    // The new chip appears in the marketplace toolbar.
    await expect(page.getByTestId("source-chip-smithery")).toBeVisible();

    // Remove it via the drawer.
    await page.getByTestId("src-remove-smithery").click();
    await expect(page.getByTestId("source-chip-smithery")).toHaveCount(0);
  });

  test("toggling source chip filters card grid", async ({ page }) => {
    // Add a remote source so we have a non-builtin chip to deselect.
    await page.getByTestId("catalog-source-settings").click();
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    // Builtin chip exists and is active by default.
    const builtin = page.getByTestId("source-chip-builtin");
    await expect(builtin).toBeVisible();
    await expect(builtin).toHaveClass(/active/);

    // Deselect builtin → builtin entries should disappear.
    await builtin.click();
    await expect(builtin).not.toHaveClass(/active/);
    await expect(
      page.getByTestId("catalog-card").filter({ hasText: "Filesystem" })
    ).toHaveCount(0);
  });

  test("validates URL when adding a source", async ({ page }) => {
    await page.getByTestId("catalog-source-settings").click();
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("bad");
    await page.getByTestId("src-name").fill("Bad");
    await page.getByTestId("src-url").fill("not-a-url");
    await page.getByTestId("src-save").click();
    await expect(page.getByText(/url must start with http/i)).toBeVisible();
    // The chip should NOT have been created.
    await expect(page.getByTestId("source-chip-bad")).toHaveCount(0);
  });
});
