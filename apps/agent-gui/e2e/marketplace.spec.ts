/**
 * E2E: MCP Marketplace — browse, filter, install (happy + runtime-missing),
 * uninstall flows backed by tauri-mock fixtures.
 *
 * Marketplace is accessed through the Settings page's MCP tab.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
  await page.goto("/");
  await page.getByTestId("nav-settings").click();
  await page.getByTestId("settings-tab-mcp").click();
  await page.getByTestId("mcp-subtab-marketplace").click();
});

test.describe("Marketplace", () => {
  test("browses the catalog and shows the filesystem entry", async ({ page }) => {
    const card = page.getByTestId("catalog-card").filter({ hasText: "Filesystem" });
    await expect(card).toBeVisible();
  });

  test("filters by keyword", async ({ page }) => {
    await page.getByTestId("catalog-search").fill("filesystem");
    await expect(page.getByTestId("catalog-card")).toHaveCount(1);
  });

  test("installs the filesystem entry happy path", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    // Wait for the install to complete (progress text shows "Install complete.").
    await expect(page.getByTestId("install-progress")).toContainText(/complete/i, {
      timeout: 10_000
    });
    await page.getByTestId("install-close").click();
    // Close the CatalogDetail drawer that still overlays the page.
    await page.locator(".drawer-close-btn").click();
    await page.getByTestId("tab-installed").click();
    await expect(page.getByTestId("uninstall-filesystem")).toBeEnabled();
  });

  test("runtime-missing path shows a missing npx hint", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error injected on window for tauri-mock to read
      window.__MARKETPLACE_FORCE_MISSING__ = ["npx"];
    });
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toContainText("Missing runtimes");
    await expect(page.getByTestId("install-progress")).toContainText("npx");
  });

  test("uninstall removes the entry", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    // Wait for the install to complete before closing.
    await expect(page.getByTestId("install-progress")).toContainText(/complete/i, {
      timeout: 10_000
    });
    await page.getByTestId("install-close").click();
    // Close the CatalogDetail drawer that still overlays the page.
    await page.locator(".drawer-close-btn").click();
    await page.getByTestId("tab-installed").click();
    await page.getByTestId("uninstall-filesystem").click();
    await expect(page.getByTestId("uninstall-filesystem")).toHaveCount(0);
  });
});

test.describe("Marketplace — Phase 2 remote catalog sources", () => {
  test("user can add and remove a remote catalog source", async ({ page }) => {
    // Open the source-settings drawer.
    await page.getByTestId("catalog-source-settings").click();
    await expect(page.getByTestId("catalog-source-settings-drawer")).toBeVisible();
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
    // Add a remote source so we have a non-builtin chip to toggle.
    await page.getByTestId("catalog-source-settings").click();
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    // The new remote chip starts active.
    const remote = page.getByTestId("source-chip-smithery");
    await expect(remote).toBeVisible();
    await expect(remote).toHaveClass(/active/);

    // Deselect the remote chip → only builtin entries remain.
    await remote.click();
    await expect(remote).not.toHaveClass(/active/);
    // Builtin chip stays active and filesystem card is still visible.
    await expect(page.getByTestId("source-chip-builtin")).toHaveClass(/active/);
    await expect(page.getByTestId("catalog-card").filter({ hasText: "Filesystem" })).toBeVisible();
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

test.describe("Settings panes backed by tauri-mock", () => {
  test("manages MCP settings server state", async ({ page }) => {
    await page.getByTestId("mcp-subtab-servers").click();
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("GitHub");
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("Enabled");

    await page.getByTestId("mcp-enable-github").click();
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("Disabled");

    await page.getByTestId("mcp-form-name").fill("Local Tools");
    await page.getByTestId("mcp-form-command").fill("node");
    await page.getByTestId("mcp-form-args").fill("server.js --stdio");
    await page.getByTestId("mcp-save-button").click();
    await expect(page.getByTestId("mcp-server-row-local-tools")).toContainText("Local Tools");

    await page.getByTestId("mcp-delete-local-tools").click();
    await expect(page.getByTestId("mcp-server-row-local-tools")).toHaveCount(0);
  });

  test("manages skill settings discovery, install, update, and delete", async ({ page }) => {
    await page.getByTestId("settings-tab-skills").click();
    await expect(page.getByTestId("skill-row-project-review")).toContainText("Project Review");
    await expect(page.getByTestId("skill-invalid-invalid-workspace-skill")).toContainText(
      "Missing required description"
    );

    await page.getByTestId("skill-enabled-project-review").click();
    await expect(page.getByTestId("skill-row-project-review")).toContainText("Disabled");

    await page.getByTestId("skill-update-registry-review").click();
    await expect(page.getByTestId("skill-row-registry-review")).toContainText("up to date");

    await page.getByTestId("skill-discover-query").fill("review");
    await page.getByTestId("skill-discover-submit").click();
    await expect(page.getByTestId("skill-remote-code-review-assistant")).toBeVisible();
    await page.getByTestId("skill-install-code-review-assistant").click();
    await expect(page.getByTestId("skill-row-code-review-assistant")).toContainText(
      "Code Review Assistant"
    );

    await page.getByTestId("skill-delete-code-review-assistant").click();
    await expect(page.getByTestId("skill-row-code-review-assistant")).toHaveCount(0);
  });
});
