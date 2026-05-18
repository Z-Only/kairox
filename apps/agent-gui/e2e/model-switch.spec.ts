import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("Mid-session model switch (P4)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // The mock only emits ContextAssembled inside send_message, so send
    // one real message to make the meter render (mirrors the P3 pattern
    // in context-meter.spec.ts).
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    await page.waitForSelector('[data-test="context-meter-ring"]', { timeout: 5_000 });
  });

  test("new session button creates a default session without opening a profile dialog", async ({
    page
  }) => {
    await page.click('[data-test="new-session-btn"]');

    await expect(page.locator('[data-test="new-session-dialog"]')).toHaveCount(0);
    await expect(page.locator('[data-test="chat-model-trigger"]')).toContainText(
      "OpenAI · GPT-4o Mini"
    );
  });

  test("chat model badge opens a selector and switches the active model", async ({ page }) => {
    await page.click('[data-test="chat-model-trigger"]');

    await expect(page.locator('[data-test="chat-model-popover"]')).toBeVisible();
    await expect(page.locator('[data-test="chat-model-option-fast"]')).toContainText(
      "OpenAI · GPT-4o Mini"
    );
    await expect(page.locator('[data-test="chat-model-option-fast"]')).toContainText(/current/i);

    await page.click('[data-test="chat-model-option-smart"]');
    await expect(page.locator('[data-test="chat-model-trigger"]')).toContainText("OpenAI · GPT-4o");

    await page.fill('[data-test="message-input"]', "after model switch");
    await page.click('[data-test="send-button"]');
    await page.click('[data-test="chat-model-trigger"]');
    await expect(page.locator('[data-test="chat-model-option-smart"]')).toContainText(/current/i);
  });

  test("reasoning-capable models expose default and custom effort choices", async ({ page }) => {
    await page.click('[data-test="chat-model-trigger"]');
    await page.hover('[data-test="chat-model-option-smart"]');

    await expect(page.locator('[data-test="chat-reasoning-panel"]')).toBeVisible();
    await expect(page.locator('[data-test="chat-reasoning-option-low"]')).toBeVisible();
    await expect(page.locator('[data-test="chat-reasoning-option-xhigh"]')).toBeVisible();

    await page.fill('[data-test="chat-reasoning-custom-input"]', "reasoning-max");
    await page.click('[data-test="chat-reasoning-custom-apply"]');

    await expect(page.locator('[data-test="chat-model-trigger"]')).toContainText("reasoning-max");
  });

  // Tests for the context-meter switch-model button were removed because
  // that feature was intentionally pulled out of the context meter popover
  // (see PR #120 / fix(gui): UI polish round 2).
});
