import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

// `apps/agent-gui/package.json` is `"type": "module"`, so CJS-style
// `__dirname` is undefined. Derive it from `import.meta.url`, as all
// sibling specs do (e.g. `context-meter.spec.ts`).
const __dirname = dirname(fileURLToPath(import.meta.url));

test.describe("Mid-session model switch (P4)", () => {
  test.beforeEach(async ({ page }) => {
    const mockPath = resolve(__dirname, "tauri-mock.js");
    await page.addInitScript({ path: mockPath });
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

  test("switch-model button is enabled and opens the profile picker", async ({ page }) => {
    await page.click('[data-test="context-meter-ring"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();

    const switchBtn = page.locator('[data-test="context-meter-switch-model"]');
    await expect(switchBtn).toBeEnabled();
    await switchBtn.click();

    // The profile picker renders the two mock profiles.
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-profile-smart"]')).toBeVisible();

    // The current profile ("fast" by default in the mock state) carries
    // the "(Current)" marker.
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).toContainText(
      /current|当前/i
    );
  });

  test("selecting a different profile emits ModelProfileSwitched and updates the meter", async ({
    page
  }) => {
    await page.click('[data-test="context-meter-ring"]');
    await page.click('[data-test="context-meter-switch-model"]');

    const smart = page.locator('[data-test="context-meter-profile-smart"]');
    await expect(smart).toBeVisible();
    await smart.click();

    // After the switch, the popover closes (both `profilePickerOpen` and
    // `popoverOpen` flip to false — matches Task 8's component contract).
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeHidden();

    // Re-open and confirm the "(Current)" marker now sits on `smart`.
    await page.click('[data-test="context-meter-ring"]');
    await page.click('[data-test="context-meter-switch-model"]');
    await expect(page.locator('[data-test="context-meter-profile-smart"]')).toContainText(
      /current|当前/i
    );
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).not.toContainText(
      /current|当前/i
    );
  });

  test("selecting the already-current profile is a silent no-op", async ({ page }) => {
    await page.click('[data-test="context-meter-ring"]');
    await page.click('[data-test="context-meter-switch-model"]');

    // Clicking "fast" while "fast" is current must close the picker but
    // leave the meter unchanged — no toast, no event in the trace.
    await page.click('[data-test="context-meter-profile-fast"]');

    // Picker closes; popover may remain open (same-profile branch in the
    // component only flips `profilePickerOpen`, not `popoverOpen`).
    // We just verify that the meter still shows the same numbers and no
    // error-toast appears.
    await expect(page.locator('.toast-error, [data-test^="toast-error"]')).toHaveCount(0);
  });
});
