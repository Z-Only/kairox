/**
 * E2E: Chat flow — send messages, see assistant response, cancel streaming.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("sends a message and sees user message immediately", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Type a message
  const input = page.locator(".message-input");
  await input.fill("Hello agent!");
  await input.press("Enter");

  // User message should appear
  await expect(page.locator(".message-user").first()).toBeVisible();
  await expect(page.locator(".message-user").first()).toContainText(
    "Hello agent!"
  );
});

test("receives streaming assistant response", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message
  const input = page.locator(".message-input");
  await input.fill("Tell me something");
  await input.press("Enter");

  // Should see streaming indicator (cursor)
  await expect(page.locator(".cursor")).toBeVisible({ timeout: 5_000 });

  // Wait for assistant message to complete
  await expect(page.locator(".message-assistant").first()).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".message-assistant").first()).toContainText(
    "mock assistant"
  );
});

test("shows cancel button while streaming and send button when idle", async ({
  page
}) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Initially, Send button should be visible
  await expect(page.locator(".send-button")).toBeVisible();
  await expect(page.locator(".cancel-button")).toBeHidden();

  // Send a message (triggers streaming)
  const input = page.locator(".message-input");
  await input.fill("Hello");
  await input.press("Enter");

  // During streaming, Cancel should appear
  await expect(page.locator(".cancel-button")).toBeVisible({ timeout: 3_000 });
  await expect(page.locator(".send-button")).toBeHidden();

  // Wait for response to complete
  await expect(page.locator(".send-button")).toBeVisible({ timeout: 10_000 });
});

test("cancels a streaming session", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message
  const input = page.locator(".message-input");
  await input.fill("Long response please");
  await input.press("Enter");

  // Click Cancel during streaming
  await expect(page.locator(".cancel-button")).toBeVisible({ timeout: 3_000 });
  await page.locator(".cancel-button").click();

  // Should show cancelled marker
  await expect(page.locator(".cancelled-marker")).toBeVisible({
    timeout: 3_000
  });
});

test("chat panel shows profile badge", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".chat-header .profile-badge")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".chat-header .profile-badge")).toContainText(
    "fast"
  );
});
