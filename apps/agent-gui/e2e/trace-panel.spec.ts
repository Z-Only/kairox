/**
 * E2E: Trace timeline — events appear in trace panel as the session progresses.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("trace panel shows events after sending a message", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message to trigger events
  const input = page.locator(".message-input");
  await input.fill("Hello");
  await input.press("Enter");

  // Trace panel should show entries after events are processed
  // The mock emits: UserMessageAdded, ContextAssembled, ModelRequestStarted, tokens, AssistantMessageCompleted
  await expect(page.locator(".trace-entry").first()).toBeVisible({
    timeout: 8_000
  });
});

test("trace entries contain event details", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message
  const input = page.locator(".message-input");
  await input.fill("What is Rust?");
  await input.press("Enter");

  // Wait for trace entries to appear
  await expect(page.locator(".trace-entry").first()).toBeVisible({
    timeout: 8_000
  });

  // Should contain at least user message trace entry
  const entries = page.locator(".trace-entry");
  const count = await entries.count();
  expect(count).toBeGreaterThanOrEqual(1);
});

test("trace panel is visible in the right sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".right-sidebar")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".trace-timeline")).toBeVisible();
});
