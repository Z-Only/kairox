import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

// R4-B: the primary in-workbench ContextMeter ring was demoted to the
// secondary ContextMeterPill mounted in the ChatComposer footer. The compaction
// signal itself now lives inline in the chat stream
// (`ChatCompactionItem`, PRs #471-#477). These selectors target the pill
// trigger; the popover content selectors (`context-meter-popover`,
// `context-meter-reserved`, `context-meter-compact`) are preserved.
test.describe("ContextMeterPill (P3)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // The mock only emits ContextAssembled inside `send_message`, so we need
    // to send a real message before the meter has any usage to render.
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    await page.waitForSelector('[data-test="context-meter-pill-trigger"]', { timeout: 5_000 });
  });

  test("renders the pill after the first message", async ({ page }) => {
    await expect(page.locator('[data-test="workbench-context-meter-pill"]')).toHaveCount(0);
    await expect(page.locator('[data-test="composer-context-meter-pill"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-pill-trigger"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-pill-pct"]')).toContainText("%");
  });

  test("opens the popover and triggers compaction", async ({ page }) => {
    await page.click('[data-test="context-meter-pill-trigger"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-reserved"]')).toBeVisible();

    await page.click('[data-test="context-meter-compact"]');
    // The pill closes the popover immediately after requesting compaction;
    // the bar-mode busy badge is not rendered in this surface.
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeHidden();
    await expect(page.locator('[data-test="context-meter-pill-trigger"]')).toBeVisible();
  });
});
