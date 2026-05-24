/**
 * E2E: Trace timeline — events appear in trace panel as the session progresses.
 */
import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

async function sendMessageAndWaitForTrace(page: Page, message = "Hello") {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  const input = page.locator('textarea[data-test="message-input"]');
  await input.fill(message);
  await input.press("Enter");

  await expect(page.getByTestId("trace-entry").first()).toBeVisible({
    timeout: 8_000
  });
}

test("trace panel shows events after sending a message", async ({ page }) => {
  await sendMessageAndWaitForTrace(page);
});

test("trace entries contain event details", async ({ page }) => {
  await sendMessageAndWaitForTrace(page, "What is Rust?");

  // Should contain at least user message trace entry
  const entries = page.getByTestId("trace-entry");
  const count = await entries.count();
  expect(count).toBeGreaterThanOrEqual(1);
});

test("filters trace events by search", async ({ page }) => {
  await sendMessageAndWaitForTrace(page, "Searchable trace request");

  // User/assistant turns are rendered directly in ChatPanel via
  // `useChatStream`; the trace store only records protocol events
  // (context, model, tool calls, permissions). See R4-A.
  const entries = page.getByTestId("trace-entry");
  await expect(entries.filter({ hasText: "context" })).toBeVisible();
  await expect(entries.filter({ hasText: "model" })).toBeVisible();

  const search = page.getByTestId("trace-search-input");
  await expect(search).toBeVisible();

  await search.fill("gpt-4o-mini");
  await expect(entries).toHaveCount(1);
  await expect(entries.filter({ hasText: "model" })).toBeVisible();

  await search.fill("history:25000");
  await expect(entries).toHaveCount(1);
  await expect(entries.filter({ hasText: "context" })).toBeVisible();

  await search.fill("does-not-exist");
  await expect(entries).toHaveCount(0);
  await expect(page.getByText("No matching trace events")).toBeVisible();

  await search.clear();
  await expect(entries.filter({ hasText: "user" })).toBeVisible();
  await expect(entries.filter({ hasText: "context" })).toBeVisible();
  await expect(entries.filter({ hasText: "model" })).toBeVisible();
});

test("trace panel is visible in the right sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".right-sidebar")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".trace-timeline")).toBeVisible();
});

test("trace tab is selectable from the right-sidebar tab group", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".trace-timeline")).toBeVisible({
    timeout: 10_000
  });
  // The tab strip highlights the active tab with an `.active` class.
  const traceTab = page.locator(".tab-group .active", { hasText: "Trace" });
  await expect(traceTab).toBeVisible();
});
