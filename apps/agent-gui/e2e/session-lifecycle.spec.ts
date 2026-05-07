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

// Selector notes after Task 7 NaiveUI migration:
//   - SessionsSidebar still renders the legacy `.session-item` (per-row) and
//     `.session-title` class hooks; the New Session dialog is still the
//     hand-rolled native <dialog class="new-session-dialog">. The Create
//     button now exposes data-test="create-session-btn".
//   - The destructive delete confirmation moved from a hand-rolled
//     `.dialog-box` ConfirmDialog to NaiveUI's portal-mounted NDialog
//     (rendered as `.n-dialog`). The Delete button is the dialog's
//     `positiveText` action.
//   - Rename / delete row buttons are now NButton wrappers; the legacy
//     `.action-btn` / `.action-delete` classes are still forwarded to the
//     NButton root so the existing selectors keep working.

test("initializes workspace on first load", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".session-item")).toHaveCount(1);
});

test("creates a new session", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Click + New button (NButton with data-test forwarded)
  await page.getByTestId("new-session-btn").click();

  // Dialog should appear
  await expect(page.locator(".new-session-dialog")).toBeVisible();

  // Click Create (the SFC tags it with data-test="create-session-btn")
  await page.getByTestId("create-session-btn").click();

  // Should now have 2 sessions
  await expect(page.locator(".session-item")).toHaveCount(2);
});

test("switches between sessions", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".session-item")).toHaveCount(1, {
    timeout: 10_000
  });

  // Create a second session
  await page.getByTestId("new-session-btn").click();
  await expect(page.locator(".new-session-dialog")).toBeVisible();
  await page.getByTestId("create-session-btn").click();
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

  // Click rename button. NButton forwards `.action-btn` to its root <button>
  // but the rename button is the *non-destructive* one (the destructive one
  // also carries `.action-delete`), so filtering by hasText pins the right
  // NButton even though both expose `.action-btn`.
  await sessionItem.locator(".action-btn", { hasText: "✏️" }).first().click();

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

  // Create a second session so we still have one after deletion
  await page.getByTestId("new-session-btn").click();
  await expect(page.locator(".new-session-dialog")).toBeVisible();
  await page.getByTestId("create-session-btn").click();
  await expect(page.locator(".session-item")).toHaveCount(2);

  // Hover over first session, click delete
  const sessionItem = page.locator(".session-item").first();
  await sessionItem.hover();
  await sessionItem.getByTestId("session-delete-btn").click();

  // The destructive confirmation is now NaiveUI's portal-mounted NDialog
  // (mounted under <NDialogProvider> in AppLayout.vue). Its root carries
  // `.n-dialog`, and the positive-text "Delete" action renders as a button
  // inside the dialog.
  const dialog = page.locator(".n-dialog");
  await expect(dialog).toBeVisible();
  await dialog.locator("button", { hasText: "Delete" }).click();

  // Should have 1 session remaining
  await expect(page.locator(".session-item")).toHaveCount(1);
});

test("status bar shows session count and mode", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("status-bar")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByTestId("status-bar")).toContainText("sessions: 1");
  await expect(page.getByTestId("status-bar")).toContainText("mode: interactive");
});
