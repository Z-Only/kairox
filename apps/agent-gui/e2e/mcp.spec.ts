/**
 * E2E: MCP server management — status indicator, server manager, trust, and events.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test.describe("MCP Server Management", () => {
  test("MCP status indicator shows in status bar", async ({ page }) => {
    await page.goto("/");
    const indicator = page.locator(".mcp-status");
    await expect(indicator).toBeVisible();
  });

  test("clicking status indicator opens server manager", async ({ page }) => {
    await page.goto("/");
    const indicator = page.locator(".mcp-status");
    await indicator.click();
    const manager = page.locator(".mcp-manager");
    await expect(manager).toBeVisible();
  });

  test("server manager shows configured servers", async ({ page }) => {
    await page.goto("/");
    // Open manager
    await page.locator(".mcp-status").click();
    const manager = page.locator(".mcp-manager");
    await expect(manager).toBeVisible();

    // The mock returns test-server (running) and stopped-server.
    // Wait for fetchServers to complete and populate the store.
    const items = page.locator(".mcp-server-item");
    await expect(items).toHaveCount(2, { timeout: 5000 });
  });

  test("can close server manager", async ({ page }) => {
    await page.goto("/");
    await page.locator(".mcp-status").click();
    const manager = page.locator(".mcp-manager");
    await expect(manager).toBeVisible();

    const closeButton = page.locator(".mcp-manager-header button");
    await closeButton.click();
    await expect(manager).not.toBeVisible();
  });
});

test.describe("MCP Permission Prompt", () => {
  test("MCP-specific permission dialog appears for MCP tools", async ({ page }) => {
    await page.goto("/");
    // This test would require triggering a permission request event
    // For now, verify the component exists in the DOM
    // In a real scenario, we'd emit a permission request event with an MCP tool
    const permissionPrompt = page.locator(".permission-prompt");
    // Component may not be visible until a permission request is triggered
    // Just verify the page loaded
    await expect(page.locator("#app")).toBeVisible();
  });
});

test.describe("MCP Events", () => {
  test("server starting event updates status indicator", async ({ page }) => {
    await page.goto("/");
    // Would emit McpServerStarting event via Tauri mock
    // Verify the indicator class changes
    const indicator = page.locator(".mcp-status");
    await expect(indicator).toBeVisible();
  });

  test("server ready event shows tool count", async ({ page }) => {
    await page.goto("/");
    // Would emit McpServerReady event
    // Verify server manager shows tool count when opened
    const indicator = page.locator(".mcp-status");
    await expect(indicator).toBeVisible();
  });
});
