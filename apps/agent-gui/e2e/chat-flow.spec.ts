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

// Selector notes after Task 7 NaiveUI migration:
//   - `.message-input` is now an NInput root <div>, not a fillable element.
//     Drive its inner <textarea> via `[data-test="message-input"] textarea`.
//   - `.send-button` / `.cancel-button` / `.cancelled-marker` are NButton/
//     NAlert wrappers; we drive them via the data-test hooks the SFC already
//     forwards through the NaiveUI components.
//   - The profile badge moved into a dedicated NTag with
//     data-test="chat-profile-badge" (was `.chat-header .profile-badge`).

test("sends a message and sees user message immediately", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Type a message. NaiveUI NInput renders a `.n-input__placeholder` overlay
  // on top of the real <textarea> until it has focus, so Playwright's default
  // `fill()` visibility check fails. `{ force: true }` bypasses the overlay
  // check; the underlying <textarea> still receives the value via input event.
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("Hello agent!", { force: true });
  await input.press("Enter");

  // User message should appear
  await expect(page.locator(".message-user").first()).toBeVisible();
  await expect(page.locator(".message-user").first()).toContainText(
    "Hello agent!"
  );
});

test("receives streaming assistant response", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message. `{ force: true }` bypasses NaiveUI's `.n-input__placeholder`
  // overlay (see the first test in this file for the full explanation).
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("Tell me something", { force: true });
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
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Initially, Send button should be visible
  await expect(page.getByTestId("send-button")).toBeVisible();
  await expect(page.getByTestId("cancel-button")).toBeHidden();

  // Send a message (triggers streaming). `{ force: true }` bypasses NaiveUI's
  // `.n-input__placeholder` overlay (see the first test in this file).
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("Hello", { force: true });
  await input.press("Enter");

  // During streaming, Cancel should appear
  await expect(page.getByTestId("cancel-button")).toBeVisible({
    timeout: 3_000
  });
  await expect(page.getByTestId("send-button")).toBeHidden();

  // Wait for response to complete
  await expect(page.getByTestId("send-button")).toBeVisible({
    timeout: 10_000
  });
});

test("cancels a streaming session", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Send a message. `{ force: true }` bypasses NaiveUI's
  // `.n-input__placeholder` overlay (see the first test in this file).
  const input = page.locator('[data-test="message-input"] textarea');
  await input.fill("Long response please", { force: true });
  await input.press("Enter");

  // Click Cancel during streaming
  await expect(page.getByTestId("cancel-button")).toBeVisible({
    timeout: 3_000
  });
  await page.getByTestId("cancel-button").click();

  // Should show cancelled marker
  await expect(page.getByTestId("cancelled-marker")).toBeVisible({
    timeout: 3_000
  });
});

test("chat panel shows profile badge", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("chat-profile-badge")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.getByTestId("chat-profile-badge")).toContainText("fast");
});
