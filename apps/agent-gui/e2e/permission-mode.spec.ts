import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("Permission mode selector", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
  });

  test("permission trigger shows default mode", async ({ page }) => {
    const trigger = page.locator('[data-test="chat-permission-trigger"]');
    await expect(trigger).toBeVisible();
    await expect(trigger).toContainText("Suggest");
  });

  test("opens the permission popover with all five options", async ({ page }) => {
    await page.click('[data-test="chat-permission-trigger"]');

    const popover = page.locator('[data-test="chat-permission-popover"]');
    await expect(popover).toBeVisible();

    const expectedModes = ["Read Only", "Suggest", "Agent", "Autonomous", "Interactive"];
    for (const label of expectedModes) {
      await expect(popover.locator("button", { hasText: label })).toBeVisible();
    }
  });

  test("marks the current permission mode as selected", async ({ page }) => {
    await page.click('[data-test="chat-permission-trigger"]');

    // Default mode is "suggest"
    const suggestOption = page.locator('[data-test="chat-permission-option-suggest"]');
    await expect(suggestOption).toHaveAttribute("aria-current", "true");

    const agentOption = page.locator('[data-test="chat-permission-option-agent"]');
    await expect(agentOption).not.toHaveAttribute("aria-current", "true");
  });

  test("selects a new permission mode and updates the trigger label", async ({ page }) => {
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-agent"]');

    const trigger = page.locator('[data-test="chat-permission-trigger"]');
    await expect(trigger).toContainText("Agent");
  });

  test("closes the popover after selection", async ({ page }) => {
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-autonomous"]');

    const popover = page.locator('[data-test="chat-permission-popover"]');
    await expect(popover).not.toBeVisible();
  });

  test("persists the selection when re-opening the popover", async ({ page }) => {
    // Select "agent"
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-agent"]');

    // Re-open and verify "agent" is the selected option
    await page.click('[data-test="chat-permission-trigger"]');
    const agentOption = page.locator('[data-test="chat-permission-option-agent"]');
    await expect(agentOption).toHaveAttribute("aria-current", "true");

    const suggestOption = page.locator('[data-test="chat-permission-option-suggest"]');
    await expect(suggestOption).not.toHaveAttribute("aria-current", "true");
  });

  test("round-trips all five modes", async ({ page }) => {
    const modes = [
      { value: "read_only", label: "Read Only" },
      { value: "suggest", label: "Suggest" },
      { value: "agent", label: "Agent" },
      { value: "autonomous", label: "Autonomous" },
      { value: "interactive", label: "Interactive" }
    ];

    for (const mode of modes) {
      await page.click('[data-test="chat-permission-trigger"]');
      await page.click(`[data-test="chat-permission-option-${mode.value}"]`);
      await expect(page.locator('[data-test="chat-permission-trigger"]')).toContainText(mode.label);
    }
  });

  test("switches mode mid-session and updates mock state", async ({ page }) => {
    // Verify initial state
    const initialMode = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentPermissionMode
    );
    expect(initialMode).toBe("suggest");

    // Switch to autonomous mode
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-autonomous"]');

    // Verify trigger label updates
    await expect(page.locator('[data-test="chat-permission-trigger"]')).toContainText("Autonomous");

    // Verify mock state reflects the change
    const updatedMode = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentPermissionMode
    );
    expect(updatedMode).toBe("autonomous");
  });

  test("persists mode in session metadata after switch", async ({ page }) => {
    // Get the current session ID
    const sessionId = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentSessionId
    );
    expect(sessionId).toBeTruthy();

    // Switch to interactive mode
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-interactive"]');

    // Verify the session object has the updated permission mode
    const sessionMode = await page.evaluate(
      ({ sid }) => {
        const state = (window as any).__KAIROX_MOCK__.state;
        const session = state.sessions.find((s: any) => s.id === sid);
        return session ? session.permission_mode : null;
      },
      { sid: sessionId }
    );
    expect(sessionMode).toBe("interactive");
  });

  test("retains mode after sending a message", async ({ page }) => {
    // Switch to agent mode
    await page.click('[data-test="chat-permission-trigger"]');
    await page.click('[data-test="chat-permission-option-agent"]');

    // Send a message
    const input = page.locator('textarea[data-test="message-input"]');
    await input.fill("Hello");
    await input.press("Enter");

    // Wait for the response to start
    await page.waitForTimeout(300);

    // Mode should still be "agent"
    await expect(page.locator('[data-test="chat-permission-trigger"]')).toContainText("Agent");

    // Mock state should still reflect "agent"
    const mode = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentPermissionMode
    );
    expect(mode).toBe("agent");
  });

  test("supports keyboard navigation for the permission selector", async ({ page }) => {
    const trigger = page.locator('[data-test="chat-permission-trigger"]');

    // Tab to the permission trigger
    await trigger.focus();
    await expect(trigger).toBeFocused();

    // Open popover with keyboard
    await trigger.press("Enter");
    const popover = page.locator('[data-test="chat-permission-popover"]');
    await expect(popover).toBeVisible();

    // Close with Escape
    await page.keyboard.press("Escape");
    await expect(popover).not.toBeVisible();
  });
});
