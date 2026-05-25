/**
 * E2E: Session lifecycle — initialize workspace, create session, switch, rename, delete.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

// Inject the Tauri mock before each test
test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

// Selector notes:
//   - SessionsSidebar renders `.session-item` (per-row) and `.session-title`
//     class hooks.
//   - The + New Session button (data-test="new-session-btn") opens an empty
//     draft composer. The session is materialized when the first message sends.
//   - The destructive delete confirmation is a row-level two-click flow:
//     `session-delete-btn` changes to `session-delete-confirm` for the same row.
//   - Rename / delete row buttons expose stable data-test attributes.

test("initializes workspace on first load", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".session-item")).toHaveCount(1);
});

test("creates a new session after the first message", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Click + New button — now opens a pending composer without adding a row.
  await page.getByTestId("new-session-btn").click();
  await expect(page.locator(".session-item")).toHaveCount(1);

  await page.getByTestId("message-input").fill("materialize a new session");
  await page.getByTestId("send-button").click();

  // First send should materialize the session and add it to the list.
  await expect(page.locator(".session-item")).toHaveCount(2);
});

test("switches between sessions", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Create a second session by opening a pending composer and sending.
  await page.getByTestId("new-session-btn").click();
  await expect(page.locator(".session-item")).toHaveCount(1);
  await page.getByTestId("message-input").fill("second session");
  await page.getByTestId("send-button").click();
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

  await sessionItem.getByTestId("session-rename-btn").click();

  // Type new name
  const input = sessionItem.locator(".rename-input");
  await expect(input).toBeVisible();
  await input.clear();
  await input.fill("My Renamed Session");
  await input.press("Enter");

  // Verify new title appears
  await expect(sessionItem.locator(".session-title")).toHaveText("My Renamed Session");
});

test("deletes a session with confirmation", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Create a second session by opening a pending composer and sending.
  await page.getByTestId("new-session-btn").click();
  await page.getByTestId("message-input").fill("second session");
  await page.getByTestId("send-button").click();
  await expect(page.locator(".session-item")).toHaveCount(2);

  // Delete the first session via two-click archive flow:
  // first click sets pending delete, second click confirms.
  const sessionItem = page.locator(".session-item").first();
  await sessionItem.hover();
  await sessionItem.getByTestId("session-archive-btn").click();
  await sessionItem.getByTestId("session-archive-btn").click();

  // Should have 1 session remaining
  await expect(page.locator(".session-item")).toHaveCount(1);
});

// The status bar test was removed because StatusBar was moved out of the
// workbench layout as part of UI polish (PR #120).
