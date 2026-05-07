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
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message to trigger events. NaiveUI NInput renders the field as a
  // <textarea> nested inside the [data-test="message-input"] root <div>.
  // `{ force: true }` bypasses NaiveUI's `.n-input__placeholder` overlay
  // (see chat-flow.spec.ts for the full explanation).
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("Hello", { force: true });
  await input.press("Enter");

  // Trace panel should show entries after events are processed
  // The mock emits: UserMessageAdded, ContextAssembled, ModelRequestStarted, tokens, AssistantMessageCompleted
  await expect(page.locator(".trace-entry").first()).toBeVisible({
    timeout: 8_000
  });
});

test("trace entries contain event details", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message. `{ force: true }` bypasses NaiveUI's
  // `.n-input__placeholder` overlay (see chat-flow.spec.ts).
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("What is Rust?", { force: true });
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

test("trace tab is selectable from the right-sidebar tab group", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".trace-timeline")).toBeVisible({
    timeout: 10_000
  });
  // The hand-rolled tab strip in TraceTimeline.vue renders NaiveUI NButtons
  // that forward `:class="{ active }"` to their root, so the active state is
  // still selectable via the legacy `.active` class hook.
  const traceTab = page.locator(".tab-group .active", { hasText: "Trace" });
  await expect(traceTab).toBeVisible();
});
