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

async function dragHorizontally(page: Page, selector: string, deltaX: number) {
  const box = await page.locator(selector).boundingBox();
  expect(box).not.toBeNull();
  const startX = box!.x + box!.width / 2;
  const startY = box!.y + box!.height / 2;

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + deltaX, startY, { steps: 5 });
  await page.mouse.up();
}

test("collapses and resizes both workbench sidebars with persisted widths", async ({ page }) => {
  await openWorkbench(page);
  await page.evaluate(() => window.localStorage.clear());
  await page.reload();
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  await page.getByTestId("left-sidebar-toggle").click();
  await expect(page.getByTestId("view-workbench")).toHaveClass(/workbench--left-collapsed/);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.left-sidebar-collapsed")))
    .toBe("true");

  await page.getByTestId("left-sidebar-toggle").click();
  await expect(page.getByTestId("view-workbench")).not.toHaveClass(/workbench--left-collapsed/);

  await dragHorizontally(page, '[data-test="left-sidebar-resizer"]', 50);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.left-sidebar-width")))
    .toBe("270");

  await dragHorizontally(page, '[data-test="right-sidebar-resizer"]', -40);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("kairox.right-sidebar-width")))
    .toBe("320");

  await page.reload();
  await expect(page.getByTestId("sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });
  await expect
    .poll(() => page.locator(".left-sidebar").boundingBox())
    .toMatchObject({
      width: 270
    });
  await expect
    .poll(() => page.locator(".right-sidebar").boundingBox())
    .toMatchObject({
      width: 320
    });
});

test("keeps project and regular session navigation in separate scroll panes", async ({ page }) => {
  await openWorkbench(page);

  await expect(page.getByTestId("projects-scroll-region")).toBeVisible();
  await expect(page.getByTestId("sessions-scroll-region")).toBeVisible();

  await expect
    .poll(() =>
      page.evaluate(() => ({
        outerOverflow: getComputedStyle(document.querySelector(".session-scroll")!).overflowY,
        projectOverflow: getComputedStyle(
          document.querySelector('[data-test="projects-scroll-region"]')!
        ).overflowY,
        sessionsOverflow: getComputedStyle(
          document.querySelector('[data-test="sessions-scroll-region"]')!
        ).overflowY,
        sectionMaxHeights: Array.from(
          document.querySelectorAll(".sessions-sidebar .sidebar-section")
        ).map((section) => getComputedStyle(section).maxHeight)
      }))
    )
    .toEqual({
      outerOverflow: "hidden",
      projectOverflow: "auto",
      sessionsOverflow: "auto",
      sectionMaxHeights: ["50%", "50%"]
    });
});
