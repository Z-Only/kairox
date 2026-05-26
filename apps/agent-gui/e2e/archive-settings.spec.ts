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

test("filters archived sessions by search", async ({ page }) => {
  await page.addInitScript(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    const projects = [
      {
        project_id: "project_archive",
        display_name: "Archive Project",
        root_path: "/mock/workspace/archive-project",
        removed_at: null,
        sort_order: 0,
        expanded: true,
        path_exists: true
      },
      {
        project_id: "project_review",
        display_name: "Review Project",
        root_path: "/mock/workspace/review-project",
        removed_at: null,
        sort_order: 1,
        expanded: true,
        path_exists: true
      }
    ];
    mock.state.projects = projects;
    mock.state.projectSessions.set("project_archive", []);
    mock.state.projectSessions.set("project_review", []);
    mock.state.archivedSessions = [
      {
        id: "ses_archived_e2e",
        title: "Archived E2E task",
        profile: "fast",
        project_id: "project_archive",
        worktree_path: "/mock/workspace/archive-project",
        branch: "fix/archive-confirm",
        deleted_at: "2026-01-02T03:04:05Z",
        visibility: "archived"
      },
      {
        id: "ses_review_e2e",
        title: "Review inbox cleanup",
        profile: "reviewer",
        project_id: "project_review",
        worktree_path: "/mock/workspace/review-project",
        branch: "feat/review-inbox",
        deleted_at: "2026-02-03T04:05:06Z",
        visibility: "archived"
      }
    ];
  });

  await openWorkbench(page);
  await page.getByTestId("nav-settings").click();
  await page.getByTestId("settings-tab-archive").click();
  await expect(page.getByTestId("archive-search-input")).toBeVisible();

  await page.getByTestId("archive-search-input").fill("review project");
  await expect(page.getByTestId("archive-row-ses_review_e2e")).toBeVisible();
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toHaveCount(0);

  await page.getByTestId("archive-search-input").fill("fix/archive-confirm");
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeVisible();
  await expect(page.getByTestId("archive-row-ses_review_e2e")).toHaveCount(0);

  await page.getByTestId("archive-search-input").fill("does-not-exist");
  await expect(page.getByTestId("archive-filter-empty")).toContainText(
    "No archived sessions match your search."
  );
  await expect(page.getByTestId("archive-list")).toHaveCount(0);

  await page.getByTestId("archive-search-input").fill("");
  await expect(page.getByTestId("archive-row-ses_archived_e2e")).toBeVisible();
  await expect(page.getByTestId("archive-row-ses_review_e2e")).toBeVisible();
});
