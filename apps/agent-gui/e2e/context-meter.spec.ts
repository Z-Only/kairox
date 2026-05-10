import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

// `apps/agent-gui/package.json` is `"type": "module"`, so the CJS-style
// `__dirname` is undefined here. Mirror the pattern used by every other
// spec in this directory (e.g. `chat-flow.spec.ts`, `session-lifecycle.spec.ts`)
// — derive `__dirname` from `import.meta.url` and load the mock by `path`
// rather than embedding its source via `readFileSync`.
const __dirname = dirname(fileURLToPath(import.meta.url));

test.describe("ContextMeter (P3)", () => {
  test.beforeEach(async ({ page }) => {
    const mockPath = resolve(__dirname, "tauri-mock.js");
    await page.addInitScript({ path: mockPath });
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
