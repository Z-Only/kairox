import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.describe("Approval policy selector", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
  });

  test("approval trigger shows default policy label", async ({ page }) => {
    const trigger = page.locator('[data-test="chat-approval-trigger"]');
    await expect(trigger).toBeVisible();
    await expect(trigger).toContainText("On Request");
  });

  test("opens the approval popover with all three options", async ({ page }) => {
    await page.click('[data-test="chat-approval-trigger"]');

    const popover = page.locator('[data-test="chat-approval-popover"]');
    await expect(popover).toBeVisible();

    const expected = ["Never", "On Request", "Always"];
    for (const label of expected) {
      await expect(popover.locator("button", { hasText: label })).toBeVisible();
    }
  });

  test("marks the current approval policy as selected", async ({ page }) => {
    await page.click('[data-test="chat-approval-trigger"]');

    const onRequest = page.locator('[data-test="chat-approval-option-on_request"]');
    await expect(onRequest).toHaveAttribute("aria-current", "true");

    const always = page.locator('[data-test="chat-approval-option-always"]');
    await expect(always).not.toHaveAttribute("aria-current", "true");
  });

  test("selects a new approval policy and updates the trigger label", async ({ page }) => {
    await page.click('[data-test="chat-approval-trigger"]');
    await page.click('[data-test="chat-approval-option-always"]');

    const trigger = page.locator('[data-test="chat-approval-trigger"]');
    await expect(trigger).toContainText("Always");
  });

  test("closes the popover after selection", async ({ page }) => {
    await page.click('[data-test="chat-approval-trigger"]');
    await page.click('[data-test="chat-approval-option-never"]');

    const popover = page.locator('[data-test="chat-approval-popover"]');
    await expect(popover).not.toBeVisible();
  });

  test("switches approval policy and updates mock state", async ({ page }) => {
    const initial = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentApprovalPolicy
    );
    expect(initial).toBe("on_request");

    await page.click('[data-test="chat-approval-trigger"]');
    await page.click('[data-test="chat-approval-option-always"]');

    await expect(page.locator('[data-test="chat-approval-trigger"]')).toContainText("Always");

    const updated = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentApprovalPolicy
    );
    expect(updated).toBe("always");
  });

  test("persists approval policy in session metadata after switch", async ({ page }) => {
    const sessionId = await page.evaluate(
      () => (window as any).__KAIROX_MOCK__.state.currentSessionId
    );
    expect(sessionId).toBeTruthy();

    await page.click('[data-test="chat-approval-trigger"]');
    await page.click('[data-test="chat-approval-option-never"]');

    const sessionApproval = await page.evaluate(
      ({ sid }) => {
        const state = (window as any).__KAIROX_MOCK__.state;
        const session = state.sessions.find((s: any) => s.id === sid);
        return session ? session.approval_policy : null;
      },
      { sid: sessionId }
    );
    expect(sessionApproval).toBe("never");
  });

  test("round-trips all three approval policies", async ({ page }) => {
    const policies = [
      { value: "never", label: "Never" },
      { value: "on_request", label: "On Request" },
      { value: "always", label: "Always" }
    ];

    for (const policy of policies) {
      await page.click('[data-test="chat-approval-trigger"]');
      await page.click(`[data-test="chat-approval-option-${policy.value}"]`);
      await expect(page.locator('[data-test="chat-approval-trigger"]')).toContainText(policy.label);
    }
  });
});
