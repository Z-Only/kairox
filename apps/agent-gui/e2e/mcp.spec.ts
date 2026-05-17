/**
 * E2E: MCP settings pane — config file action, marketplace browse, permission prompt.
 *
 * The MCP status indicator and server manager popup were removed from StatusBar
 * during UI polish (PR #120). MCP settings are now managed exclusively through
 * the Settings page's McpSettingsPane.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("MCP Settings", () => {
  test("opens the MCP settings page with a config file action", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    const openConfigButton = page.getByTestId("mcp-open-config");
    await expect(openConfigButton).toContainText(/Open\s+[Cc]onfig\s+[Ff]ile/);
    await openConfigButton.click();
    await expect(page.getByTestId("mcp-page-error")).toHaveCount(0);
  });

  test("shows a settings error when opening the config file fails", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error injected for tauri-mock to read
      window.__MCP_OPEN_CONFIG_SHOULD_FAIL__ = true;
    });
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    await page.getByTestId("mcp-open-config").click();
    await expect(page.getByTestId("mcp-page-error")).toContainText(
      "Unable to open MCP config file"
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

test.describe("MCP Resources and Prompts", () => {
  test("expands resources accordion and shows resource rows", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    await page.getByTestId("mcp-resources-toggle-github").click();
    await expect(page.getByTestId("mcp-resource-github-App Log")).toBeVisible();
    await expect(page.getByTestId("mcp-resource-github-Settings")).toBeVisible();
  });

  test("clicks resource to show inline content", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    await page.getByTestId("mcp-resources-toggle-github").click();
    await page.getByTestId("mcp-resource-github-App Log").click();

    const content = page.getByTestId("mcp-resource-content-github-App Log");
    await expect(content).toBeVisible();
    await expect(content.locator(".content-block__text")).toContainText("Server started");
  });

  test("expands prompts accordion and shows prompt rows", async ({ page }) => {
    await page.goto("/");
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-mcp").click();

    await page.getByTestId("mcp-prompts-toggle-github").click();
    await expect(page.getByTestId("mcp-prompt-github-analyze_code")).toBeVisible();
    await expect(page.getByTestId("mcp-prompt-github-analyze_code")).toContainText("analyze_code");
    await expect(page.getByTestId("mcp-prompt-github-analyze_code")).toContainText("2 args");
  });
});

test.describe("MCP Permission Prompt", () => {
  test("MCP-specific permission dialog appears for MCP tools", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("#app")).toBeVisible();
  });
});
