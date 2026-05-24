/**
 * E2E: Inline chat-stream behaviour shipped across PRs #471–#477.
 *
 * Locks in four guarantees of the unified ChatPanel feed:
 *
 *   A. Messages and tool-call rows render together in the same chat
 *      container. `useChatStream` orders messages first (projection
 *      order) and trace items after them (sorted by `startedAt`); this
 *      test pins that deterministic order so future reorders are caught.
 *
 *   B. A permission prompt rendered inline by `ChatPermissionItem` can
 *      be accepted via its Allow button and disappears from the stream
 *      once `useChatStream` filters resolved permissions.
 *
 *   C. `ChatToolCallItem` starts collapsed; clicking the toggle button
 *      reveals the output preview and the duration label.
 *
 *   D. `ChatCompactionItem` renders only when `session.projection.
 *      compaction.type !== "Idle"`, transitioning from "running" to
 *      "completed" as the mock drives the projection compaction status.
 */
import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/");
  await page.waitForSelector('[data-test="chat-panel"]');
});

async function waitForSessionReady(page: Page): Promise<void> {
  await expect
    .poll(() => page.evaluate(() => (window as any).__KAIROX_MOCK__.state.currentSessionId))
    .not.toBeNull();
}

test("A. messages and tool-call row render together in chat stream", async ({ page }) => {
  await waitForSessionReady(page);

  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    mock.simulateUserMessage("Please list /tmp");
    mock.simulateToolInvocation("shell", "drwxrwxrwt 12 root root 384B Apr 1 12:34 /tmp", 220);
    mock.simulateAssistantMessage("Listed /tmp for you.");
  });

  const messageList = page.locator('[data-test="message-list"]');
  await expect(messageList).toBeVisible();

  const chatMessages = messageList.getByTestId("chat-message");
  await expect(chatMessages).toHaveCount(2);
  await expect(chatMessages.nth(0)).toHaveAttribute("data-role", "user");
  await expect(chatMessages.nth(0)).toContainText("Please list /tmp");
  await expect(chatMessages.nth(1)).toHaveAttribute("data-role", "assistant");
  await expect(chatMessages.nth(1)).toContainText("Listed /tmp for you.");

  // Messages render first, tool calls after — and only real tool calls.
  const toolRows = messageList.getByTestId("chat-tool-call-item");
  await expect(toolRows).toHaveCount(1);
  await expect(toolRows.nth(0)).toContainText("shell");

  const orderedKinds = await messageList.evaluate((root) => {
    const items = Array.from(
      root.querySelectorAll(
        '[data-test="chat-message"], [data-test="chat-tool-call-item"], [data-test="chat-permission-item"], [data-test="chat-compaction-item"]'
      )
    );
    return items.map((node) => node.getAttribute("data-test"));
  });
  expect(orderedKinds.slice(0, 2)).toEqual(["chat-message", "chat-message"]);
  expect(orderedKinds.filter((k) => k === "chat-tool-call-item")).toHaveLength(1);
});

test("B. inline permission prompt resolves when accepted", async ({ page }) => {
  await waitForSessionReady(page);

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulatePermissionRequest("shell", "Run `rm /tmp/x`?");
  });

  const permissionItem = page.locator('[data-test="chat-permission-item"]');
  await expect(permissionItem).toBeVisible();
  await expect(permissionItem).toHaveAttribute("data-variant", "tool");
  await expect(permissionItem.locator('[data-test="permission-prompt"]')).toContainText(
    "Run `rm /tmp/x`?"
  );

  await permissionItem.locator('[data-test="permission-allow"]').click();

  // Once granted, useChatStream filters resolved permissions out of the
  // chat-stream feed.
  await expect(permissionItem).toHaveCount(0);
});

test("C. tool-call row starts collapsed and reveals output + duration on toggle", async ({
  page
}) => {
  await waitForSessionReady(page);

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateToolInvocation(
      "shell",
      "/Users/mock\nhome\nopt\n",
      1700
    );
  });

  const toolRow = page.locator('[data-test="chat-tool-call-item"]');
  await expect(toolRow).toBeVisible();
  await expect(toolRow).toHaveClass(/chat-tool-call--completed/);

  // Duration is rendered in the always-visible header row (1.7s).
  await expect(toolRow.locator(".chat-tool-call__duration")).toHaveText("1.7s");

  // The detail section is gated on `v-if="isExpanded"`; before toggle
  // it does not exist in the DOM at all.
  await expect(toolRow.locator(".chat-tool-call__detail")).toHaveCount(0);
  await expect(toolRow.locator(".chat-tool-code")).toHaveCount(0);

  await toolRow.locator('[data-test="chat-tool-call-toggle"]').click();

  await expect(toolRow.locator(".chat-tool-call__detail")).toBeVisible();
  await expect(toolRow.locator(".chat-tool-code").first()).toContainText("/Users/mock");
});

test("D. compaction item is visible while running and after completion", async ({ page }) => {
  await waitForSessionReady(page);

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateCompactionStatus({ type: "Running" });
  });

  const compactionItem = page.locator('[data-test="chat-compaction-item"]');
  await expect(compactionItem).toBeVisible();
  await expect(compactionItem).toHaveAttribute("data-status", "running");
  await expect(compactionItem.locator('[data-test="chat-compaction-bar"]')).toBeVisible();

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateCompactionStatus({ type: "Completed" });
  });

  await expect(compactionItem).toHaveAttribute("data-status", "completed");
  await expect(compactionItem.locator('[data-test="chat-compaction-bar"]')).toHaveCount(0);
});
