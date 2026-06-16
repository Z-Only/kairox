/**
 * E2E: Inline chat-stream behaviour shipped across PRs #471–#477.
 *
 * Locks in four guarantees of the unified ChatPanel feed:
 *
 *   A. Messages and tool-call rows render together in the same chat
 *      container. Trace items are grouped with the turn and render before
 *      the assistant output they explain, so the process is visible while
 *      reading the response.
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

  // Trace items render before the assistant output they explain.
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
  expect(orderedKinds.slice(0, 3)).toEqual(["chat-message", "chat-tool-call-item", "chat-message"]);
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

test("C2. tool-call row is keyboard-accessible: Tab focus + Enter/Space toggle", async ({
  page
}) => {
  await waitForSessionReady(page);

  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateToolInvocation("shell", "echo from kbd\n", 900);
  });

  const toolRow = page.locator('[data-test="chat-tool-call-item"]');
  await expect(toolRow).toBeVisible();

  const row = toolRow.locator(".chat-tool-call__row");
  // a11y contract: clickable header is a focusable button-like control.
  await expect(row).toHaveAttribute("role", "button");
  await expect(row).toHaveAttribute("tabindex", "0");
  await expect(row).toHaveAttribute("aria-expanded", "false");

  // Enter dispatched on the row triggers the same toggle path the
  // keyboard would take. We dispatch via DOM event so this exercises
  // the @keydown handler directly without depending on Playwright's
  // focus heuristics for non-button focusables.
  await row.dispatchEvent("keydown", { key: "Enter", bubbles: true });
  await expect(toolRow.locator(".chat-tool-call__detail")).toBeVisible();
  await expect(row).toHaveAttribute("aria-expanded", "true");

  // The detail panel id matches the row's aria-controls.
  const controls = await row.getAttribute("aria-controls");
  expect(controls).toBeTruthy();
  await expect(toolRow.locator(".chat-tool-call__detail")).toHaveAttribute("id", controls!);

  // Space collapses again (and must not scroll the page).
  await row.dispatchEvent("keydown", { key: " ", bubbles: true });
  await expect(toolRow.locator(".chat-tool-call__detail")).toHaveCount(0);
  await expect(row).toHaveAttribute("aria-expanded", "false");
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

test("E. structured task confirmation resolves selected options and custom text", async ({
  page
}) => {
  await waitForSessionReady(page);

  const requestId = await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    return mock.simulateTaskConfirmationRequest(
      "Pick the regression coverage to add",
      [
        {
          id: "browser",
          label: "Browser flow",
          description: "Exercise the chat stream with the Tauri mock"
        },
        {
          id: "unit",
          label: "Unit mapping",
          description: "Assert the composable mapping only"
        },
        {
          id: "skip",
          label: "Skip coverage",
          description: null
        }
      ],
      true,
      true
    );
  });

  const card = page.getByTestId("chat-task-confirmation-item");
  await expect(card).toBeVisible();
  await expect(card).toContainText("Pick the regression coverage to add");
  await expect(card).toContainText("Browser flow");
  await expect(card).toContainText("Exercise the chat stream with the Tauri mock");
  await expect(card).toContainText("Unit mapping");
  await expect(card.getByTestId("task-confirmation-custom")).toBeVisible();

  await card.getByTestId("task-confirmation-option-browser").check();
  await card.getByTestId("task-confirmation-option-unit").check();
  await card.getByTestId("task-confirmation-custom").fill("Keep it test-only and narrow.");
  await card.getByTestId("task-confirmation-submit").click();

  await expect
    .poll(() =>
      page.evaluate(() => {
        const mock = (window as any).__KAIROX_MOCK__;
        return mock.commandCalls("resolve_task_confirmation").at(-1) ?? null;
      })
    )
    .toEqual({
      command: "resolve_task_confirmation",
      args: {
        decision: {
          request_id: requestId,
          selected_option_ids: ["browser", "unit"],
          custom_response: "Keep it test-only and narrow."
        }
      }
    });

  await expect
    .poll(() =>
      page.evaluate((id) => {
        const mock = (window as any).__KAIROX_MOCK__;
        return mock.state.taskConfirmationRequests.has(id);
      }, requestId)
    )
    .toBe(false);

  await expect(card).toHaveCount(0);
});
