import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("Sandbox policy selector", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
  });

  test("sandbox trigger shows default policy label parsed from JSON", async ({ page }) => {
    const trigger = page.locator('[data-test="chat-sandbox-trigger"]');
    await expect(trigger).toBeVisible();
    await expect(trigger).toContainText("Workspace Write");
  });

  test("opens the sandbox popover with all three options", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');

    const popover = page.locator('[data-test="chat-sandbox-popover"]');
    await expect(popover).toBeVisible();

    const expected = ["Read Only", "Workspace Write", "Danger Full Access"];
    for (const label of expected) {
      await expect(popover.locator("button", { hasText: label })).toBeVisible();
    }
  });

  test("marks the current sandbox kind as selected", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');

    const workspace = page.locator('[data-test="chat-sandbox-option-workspace_write"]');
    await expect(workspace).toHaveAttribute("aria-current", "true");

    const readOnly = page.locator('[data-test="chat-sandbox-option-read_only"]');
    await expect(readOnly).not.toHaveAttribute("aria-current", "true");
  });

  test("selects a new sandbox policy and updates the trigger label", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-read_only"]');

    const trigger = page.locator('[data-test="chat-sandbox-trigger"]');
    await expect(trigger).toContainText("Read Only");
  });

  test("closes the popover after selection", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-danger_full_access"]');

    const popover = page.locator('[data-test="chat-sandbox-popover"]');
    await expect(popover).not.toBeVisible();
  });

  test("switches sandbox policy and updates mock state to canonical JSON", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-read_only"]');

    await expect(page.locator('[data-test="chat-sandbox-trigger"]')).toContainText("Read Only");

    const updated = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentSandboxPolicy
    );
    expect(updated).toBe('{"kind":"read_only"}');
  });

  test("persists sandbox policy in session metadata after switch", async ({ page }) => {
    const sessionId = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentSessionId
    );
    expect(sessionId).toBeTruthy();

    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-danger_full_access"]');

    const sessionSandbox = await page.evaluate(
      ({ sid }) => {
        const state = (window as any).__KAIROX_MOCK__.state;
        const session = state.sessions.find((s: any) => s.id === sid);
        return session ? session.sandbox_policy : null;
      },
      { sid: sessionId }
    );
    expect(sessionSandbox).toBe('{"kind":"danger_full_access"}');
  });

  test("workspace_write JSON includes network_access and writable_roots", async ({ page }) => {
    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-read_only"]');
    await page.click('[data-test="chat-sandbox-trigger"]');
    await page.click('[data-test="chat-sandbox-option-workspace_write"]');

    const json = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentSandboxPolicy
    );
    const parsed = JSON.parse(json);
    expect(parsed.kind).toBe("workspace_write");
    expect(parsed.network_access).toBe(false);
    expect(Array.isArray(parsed.writable_roots)).toBe(true);
  });

  test("round-trips all three sandbox policies", async ({ page }) => {
    const policies = [
      { value: "read_only", label: "Read Only" },
      { value: "workspace_write", label: "Workspace Write" },
      { value: "danger_full_access", label: "Danger Full Access" }
    ];

    for (const policy of policies) {
      await page.click('[data-test="chat-sandbox-trigger"]');
      await page.click(`[data-test="chat-sandbox-option-${policy.value}"]`);
      await expect(page.locator('[data-test="chat-sandbox-trigger"]')).toContainText(policy.label);
    }
  });
});
