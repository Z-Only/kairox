import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("ContextMeter (P3)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // The mock only emits ContextAssembled inside `send_message`, so we need
    // to send a real message before the meter has any usage to render.
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    await page.waitForSelector('[data-test="context-meter-ring"]', { timeout: 5_000 });
  });

  test("renders the meter after the first message", async ({ page }) => {
    await expect(page.locator('[data-test="context-meter"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-ring"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-ring"]')).toContainText("%");
  });

  test("opens the popover and triggers compaction", async ({ page }) => {
    await page.click('[data-test="context-meter-ring"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-reserved"]')).toBeVisible();

    await page.click('[data-test="context-meter-compact"]');
    // Ring mode closes the popover immediately after requesting compaction;
    // the bar-mode busy badge is not rendered in the composer ring.
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeHidden();
    await expect(page.locator('[data-test="context-meter-ring"]')).toBeVisible();
  });
});
