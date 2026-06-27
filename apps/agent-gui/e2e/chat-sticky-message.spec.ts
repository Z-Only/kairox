import { test, expect, type Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

async function openWorkbench(page: Page) {
  await page.goto("/");
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
}

test("clicking the pinned user message scrolls back to its chat turn", async ({ page }) => {
  await openWorkbench(page);
  await page.addStyleTag({
    content: `
      .message-list {
        flex: 0 0 220px !important;
        height: 220px !important;
      }
      .message :is(.message-content, .user-message-wrapper) {
        min-height: 96px !important;
      }
      .message-assistant .message-content {
        min-height: 360px !important;
      }
    `
  });
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.setResponseDelayScale(0.1);
  });

  const input = page.getByTestId("message-input");
  await input.fill("first sticky prompt");
  await input.press("Enter");
  await expect(page.getByTestId("send-button")).toContainText("Send");

  await input.fill("second sticky prompt");
  await input.press("Enter");
  await expect(page.getByTestId("send-button")).toContainText("Send");

  const userMessages = page.getByTestId("chat-message").filter({ hasText: "sticky prompt" });
  await expect(userMessages).toHaveCount(2);

  await page.locator("[data-test='message-list']").evaluate((el) => {
    const list = el as HTMLElement;
    const first = Array.from(
      document.querySelectorAll("[data-test='chat-message'][data-role='user']")
    ).find((message) => message.textContent?.includes("first sticky prompt"));
    if (!(first instanceof HTMLElement)) {
      throw new Error("missing first user message");
    }
    first.scrollIntoView({ block: "start" });
    const firstHeight = first.getBoundingClientRect().height;
    list.scrollTo({
      top: list.scrollTop + firstHeight + 24,
      behavior: "auto"
    });
  });
  await expect(page.getByTestId("pinned-user-message")).toContainText("first sticky prompt");

  const beforeJumpScrollTop = await page
    .locator("[data-test='message-list']")
    .evaluate((el) => (el as HTMLElement).scrollTop);
  expect(beforeJumpScrollTop).toBeGreaterThan(0);

  await page.getByTestId("pinned-user-message").click();

  await expect
    .poll(() =>
      page.locator("[data-test='message-list']").evaluate((el) => {
        return (el as HTMLElement).scrollTop;
      })
    )
    .toBeLessThan(beforeJumpScrollTop - 20);
});
