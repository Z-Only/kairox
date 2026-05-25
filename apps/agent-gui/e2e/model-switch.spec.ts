import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("Mid-session model switch (P4)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // The mock only emits ContextAssembled inside send_message, so send
    // one real message to make the meter render (mirrors the P3 pattern
    // in context-meter.spec.ts). R4-B demoted the primary ring to the
    // ContextMeterPill in WorkbenchView, so wait for that selector.
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    await page.waitForSelector('[data-test="context-meter-pill-trigger"]', { timeout: 5_000 });
  });

  test("new session button opens an empty composer without a profile dialog", async ({ page }) => {
    const sessionRows = page.locator(".session-item");
    const initialSessionCount = await sessionRows.count();

    await page.click('[data-test="new-session-btn"]');

    await expect(page.locator('[data-test="new-session-dialog"]')).toHaveCount(0);
    await expect(sessionRows).toHaveCount(initialSessionCount);
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

  test("Claude profiles expose reasoning effort choices from profile metadata", async ({
    page
  }) => {
    await page.click('[data-test="chat-model-trigger"]');
    await page.hover('[data-test="chat-model-option-claude"]');

    await expect(page.locator('[data-test="chat-model-option-claude"]')).toContainText(
      "Anthropic · Claude Sonnet 4 20250514"
    );
    await expect(page.locator('[data-test="chat-reasoning-panel"]')).toBeVisible();

    await page.click('[data-test="chat-reasoning-option-high"]');

    await expect(page.locator('[data-test="chat-model-trigger"]')).toContainText(
      "Anthropic · Claude Sonnet 4 20250514 · high"
    );
  });

  // Tests for the context-meter switch-model button were removed because
  // that feature was intentionally pulled out of the context meter popover
  // (see PR #120 / fix(gui): UI polish round 2).
});
