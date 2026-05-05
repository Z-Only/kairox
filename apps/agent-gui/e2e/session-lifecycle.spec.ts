/**
 * E2E: Session lifecycle — initialize workspace, create session, switch, rename, delete.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Inject the Tauri mock before each test
test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("initializes workspace on first load", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".session-item")).toHaveCount(1);
});

test("creates a new session", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Click + New button
  await page.locator(".new-session-btn").click();

  // Dialog should appear
  await expect(page.locator(".new-session-dialog")).toBeVisible();

  // Click Create
  await page
    .locator(".new-session-dialog >> button", { hasText: "Create" })
    .click();

  // Should now have 2 sessions
  await expect(page.locator(".session-item")).toHaveCount(2);
});

test("switches between sessions", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Create a second session
  await page.locator(".new-session-btn").click();
  await expect(page.locator(".new-session-dialog")).toBeVisible();
  await page
    .locator(".new-session-dialog >> button", { hasText: "Create" })
    .click();
  await expect(page.locator(".session-item")).toHaveCount(2);

  // Second session should be active (last created)
  const items = page.locator(".session-item");
  await expect(items.nth(1)).toHaveClass(/active/);

  // Click the first session
  await items.nth(0).click();

  // First session should now be active
  await expect(items.nth(0)).toHaveClass(/active/);
});

test("renames a session", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Hover over session to reveal action buttons
  const sessionItem = page.locator(".session-item").first();
  await sessionItem.hover();

  // Click rename button
  await sessionItem.locator(".action-btn", { hasText: "✏️" }).click();

  // Type new name
  const input = sessionItem.locator(".rename-input");
  await expect(input).toBeVisible();
  await input.clear();
  await input.fill("My Renamed Session");
  await input.press("Enter");

  // Verify new title appears
  await expect(sessionItem.locator(".session-title")).toHaveText(
    "My Renamed Session"
  );
});

test("deletes a session with confirmation", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Create a second session so we still have one after deletion
  await page.locator(".new-session-btn").click();
  await expect(page.locator(".new-session-dialog")).toBeVisible();
  await page
    .locator(".new-session-dialog >> button", { hasText: "Create" })
    .click();
  await expect(page.locator(".session-item")).toHaveCount(2);

  // Hover over first session, click delete
  const sessionItem = page.locator(".session-item").first();
  await sessionItem.hover();
  await sessionItem.locator(".action-delete").click();

  // ConfirmDialog should appear (uses .dialog-box class)
  await expect(page.locator(".dialog-box")).toBeVisible();
  await page.locator(".dialog-box >> button", { hasText: "Delete" }).click();

  // Should have 1 session remaining
  await expect(page.locator(".session-item")).toHaveCount(1);
});

test("status bar shows session count and mode", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".status-bar")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".status-bar")).toContainText("sessions: 1");
  await expect(page.locator(".status-bar")).toContainText("mode: interactive");
});
