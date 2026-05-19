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

test("permanent archive delete uses the app confirm dialog", async ({ page }) => {
  await openWorkbench(page);
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const project = {
      project_id: "project_archive",
      display_name: "Archive Project",
      root_path: "/mock/workspace/archive-project",
      removed_at: null,
      sort_order: 0,
      expanded: true,
      path_exists: true
    };
    const session = {
      id: "ses_archived_e2e",
      title: "Archived E2E task",
      profile: "fast",
      permission_mode: null,
      project_id: project.project_id,
      worktree_path: project.root_path,
      branch: "fix/archive-confirm",
      visibility: "archived"
    };
    mock.state.projects = [project];
    mock.state.projectSessions.set(project.project_id, []);
    mock.state.archivedSessions = [session];
  });

  await page.getByTestId("nav-settings").click();
  await page.getByTestId("settings-tab-archive").click();
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeVisible();

  await page.getByTestId("archive-delete-ses_archived_e2e").click();
  await expect(page.getByTestId("confirm-ok")).toBeVisible();
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeVisible();

  await page.getByTestId("confirm-cancel").click();
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeVisible();

  await page.getByTestId("archive-delete-ses_archived_e2e").click();
  await page.getByTestId("confirm-ok").click();
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeHidden();
  await expect
    .poll(() => page.evaluate(() => (window as any).__KAIROX_MOCK__.state.archivedSessions.length))
    .toBe(0);
});
