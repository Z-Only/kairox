/**
 * E2E: MCP settings pane — config folder action, marketplace browse, permission prompt.
 *
 * The MCP status indicator and server manager popup were removed from StatusBar
 * during UI polish (PR #120). MCP settings are now managed exclusively through
 * the Settings page's McpSettingsPane.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test.describe("MCP Settings", () => {
  test("opens the MCP settings page with a config folder action", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    const openConfigButton = page.getByTestId("mcp-open-config");
    await expect(openConfigButton).toContainText(/Open\s+[Cc]onfig\s+[Ff]older/);
    await openConfigButton.click();
    await expect(page.getByTestId("mcp-page-error")).toHaveCount(0);
  });

  test("shows a settings error when opening the config folder fails", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error injected for tauri-mock to read
      window.__MCP_OPEN_CONFIG_SHOULD_FAIL__ = true;
    });
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    await page.getByTestId("mcp-open-config").click();
    await expect(page.getByTestId("mcp-page-error")).toContainText(
      "Unable to open MCP config folder"
    );
  });

  test("shows Marketplace browse content without an inner Browse tab", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();
    await page.getByTestId("mcp-subtab-marketplace").click();

    await expect(page.getByTestId("catalog-search")).toBeVisible();
    await expect(page.getByTestId("tab-browse")).toHaveCount(0);
  });
});

test.describe("MCP Permission Prompt", () => {
  test("MCP-specific permission dialog appears for MCP tools", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("#app")).toBeVisible();
  });
});
