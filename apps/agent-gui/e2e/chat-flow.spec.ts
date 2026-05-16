/**
 * E2E: Chat flow — send messages, see assistant response, cancel streaming.
 */
import { test, expect, type Page } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

async function openWorkbench(page: Page) {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
}

async function getMockSessionIds(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (window as any).__KAIROX_MOCK__.state.sessions.map((session: { id: string }) => session.id)
  );
}

async function waitForActiveSession(page: Page, sessionId: string) {
  await expect
    .poll(() => page.evaluate(() => (window as any).__KAIROX_MOCK__.state.currentSessionId))
    .toBe(sessionId);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.last-active-session-id")))
    .toBe(sessionId);
}

async function waitForDraft(page: Page, sessionId: string, draftText: string) {
  await expect
    .poll(() =>
      page.evaluate(
        ({ sessionId }) => (window as any).__KAIROX_MOCK__.state.drafts.get(sessionId) || "",
        { sessionId }
      )
    )
    .toBe(draftText);
}

// Selector notes:
//   - The message input is a plain <textarea data-test="message-input">.
//   - `.send-button` / `.cancel-button` / `.cancelled-marker` are driven
//     via their data-test attributes.
//   - The profile badge uses data-test="chat-model-trigger".

test("sends a message and sees user message immediately", async ({ page }) => {
  await openWorkbench(page);

  // Type a message into the plain <textarea>.
  const input = page.locator('textarea[data-test="message-input"]');
  await input.fill("Hello agent!");
  await input.press("Enter");

  // User message should appear
  await expect(page.locator(".message-user").first()).toBeVisible();
  await expect(page.locator(".message-user").first()).toContainText("Hello agent!");
});

test("receives streaming assistant response", async ({ page }) => {
  await openWorkbench(page);

  // Send a message.
  const input = page.locator('textarea[data-test="message-input"]');
  await input.fill("Tell me something");
  await input.press("Enter");

  // Should see streaming indicator (cursor)
  await expect(page.locator(".cursor")).toBeVisible({ timeout: 5_000 });

  // Wait for assistant message to complete
  await expect(page.locator(".message-assistant").first()).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".message-assistant").first()).toContainText("mock assistant");
});

test("shows cancel button while streaming and send button when idle", async ({ page }) => {
  await openWorkbench(page);

  // Initially, Send button should be visible
  await expect(page.getByTestId("send-button")).toBeVisible();
  await expect(page.getByTestId("cancel-button")).toBeHidden();

  // Send a message (triggers streaming).
  const input = page.locator('textarea[data-test="message-input"]');
  await input.fill("Hello");
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
  await openWorkbench(page);

  // Send a message.
  const input = page.locator('textarea[data-test="message-input"]');
  await input.fill("Long response please");
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
  await openWorkbench(page);
  await expect(page.getByTestId("chat-model-trigger")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.getByTestId("chat-model-trigger")).toContainText("OpenAI");
});

test("opens slash command palette and inserts command via keyboard selection", async ({ page }) => {
  await openWorkbench(page);

  const input = page.getByTestId("message-input");
  await input.fill("/");

  await expect(page.getByTestId("command-palette")).toBeVisible();
  await expect(page.getByTestId("palette-item-model")).toBeVisible();

  await input.press("ArrowDown");
  await input.press("ArrowDown");
  await input.press("Enter");

  await expect(page.getByTestId("command-palette")).toBeHidden();
  await expect(input).toHaveValue("/model ");
});

test("opens file mention palette and selects a workspace file via keyboard", async ({ page }) => {
  await openWorkbench(page);

  await page.getByTestId("project-create-trigger").click();
  await page.getByTestId("project-create-blank").click();
  await page.getByTestId("project-new-session-btn").first().click();

  const input = page.getByTestId("message-input");
  await input.fill("@chat");

  await expect(page.getByTestId("file-mention-palette")).toBeVisible();
  await expect(page.getByTestId("mention-file-item").first()).toContainText(
    "apps/agent-gui/src/components/ChatComposer.vue"
  );

  await input.press("Enter");

  await expect(page.getByTestId("file-mention-palette")).toBeHidden();
  await expect(input).toHaveValue("@apps/agent-gui/src/components/ChatComposer.vue ");
});

test("restores each session draft when switching sessions", async ({ page }) => {
  await openWorkbench(page);

  const input = page.getByTestId("message-input");
  const sessions = page.locator(".session-item");

  await page.getByTestId("new-session-btn").click();
  await expect(sessions).toHaveCount(2);
  const [, secondSessionId] = await getMockSessionIds(page);
  await expect(sessions.nth(1)).toHaveClass(/active/);
  await waitForActiveSession(page, secondSessionId);

  await input.fill("draft for the second session");
  await waitForDraft(page, secondSessionId, "draft for the second session");
  await sessions.nth(0).click();
  const [firstSessionId] = await getMockSessionIds(page);
  await waitForActiveSession(page, firstSessionId);
  await expect(sessions.nth(0)).toHaveClass(/active/);
  await expect(input).toHaveValue("");

  await sessions.nth(1).click();
  await waitForActiveSession(page, secondSessionId);
  await expect(sessions.nth(1)).toHaveClass(/active/);
  await expect(input).toHaveValue("draft for the second session");
});

test("recovers the active session and its draft after reload", async ({ page }) => {
  await openWorkbench(page);

  const input = page.getByTestId("message-input");
  const sessions = page.locator(".session-item");

  await page.getByTestId("new-session-btn").click();
  await expect(sessions).toHaveCount(2);
  const [, secondSessionId] = await getMockSessionIds(page);
  await expect(sessions.nth(1)).toHaveClass(/active/);
  await waitForActiveSession(page, secondSessionId);

  await input.fill("draft that survives reload");
  await waitForDraft(page, secondSessionId, "draft that survives reload");

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.persistForReload();
  });
  await page.reload();

  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect(page.locator(".session-item")).toHaveCount(2);
  await expect(page.locator(".session-item").nth(1)).toHaveClass(/active/);

  const [firstSessionId] = await getMockSessionIds(page);
  await page.locator(".session-item").nth(0).click();
  await waitForActiveSession(page, firstSessionId);
  await expect(page.getByTestId("message-input")).toHaveValue("");

  await page.locator(".session-item").nth(1).click();
  await waitForActiveSession(page, secondSessionId);
  await expect(page.getByTestId("message-input")).toHaveValue("draft that survives reload");
});
