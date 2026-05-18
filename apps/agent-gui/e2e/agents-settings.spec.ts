import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test.describe("Agents Settings", () => {
  async function navigateToAgents(page: Page) {
    await page.goto("/");
    await page.waitForSelector('[data-test="nav-settings"]', { timeout: 10000 });
    await page.getByTestId("nav-settings").click();
    await page.getByTestId("settings-tab-agents").click();
    await expect(page.getByTestId("agent-settings-pane")).toBeVisible();
  }

  function agentRow(page: Page, name: string, scope: "Builtin" | "User" | "Project") {
    return page.locator(`[data-test="agent-row-${name}"][data-agent-scope="${scope}"]`);
  }

  test("renders agents and edits a user-scoped agent", async ({ page }) => {
    await navigateToAgents(page);

    await expect(agentRow(page, "worker", "Builtin")).toContainText("Built-in");
    await expect(agentRow(page, "code-reviewer", "User")).toContainText("smart");

    await page.getByTestId("agent-edit-code-reviewer").click();
    await expect(page.getByTestId("agent-form-name")).toHaveValue("code-reviewer");
    await page.getByTestId("agent-form-description").fill("Review diffs before handoff.");
    await page.getByTestId("agent-form-model").fill("fast");
    await page.getByTestId("agent-form-permission").fill("workspace_write");
    await page.getByTestId("agent-form-tools").fill("fs.read, search, shell");
    await page.getByTestId("agent-form-skills").fill("kairox-dev-workflow, test-driven-rust");
    await page.getByTestId("agent-form-nicknames").fill("Reviewer, Audit");
    await page
      .getByTestId("agent-form-instructions")
      .fill("Lead with concrete findings.\nCall out missing tests.");
    await page.getByTestId("agent-save").click();

    await expect(agentRow(page, "code-reviewer", "User")).toContainText(
      "Review diffs before handoff."
    );
    await expect(agentRow(page, "code-reviewer", "User")).toContainText("fast");
    await expect(agentRow(page, "code-reviewer", "User")).toContainText("workspace_write");
    await expect(agentRow(page, "code-reviewer", "User")).toContainText("shell");

    await page.getByTestId("agent-edit-code-reviewer").click();
    await expect(page.getByTestId("agent-form-model")).toHaveValue("fast");
    await expect(page.getByTestId("agent-form-permission")).toHaveValue("workspace_write");
    await expect(page.getByTestId("agent-form-skills")).toHaveValue(
      "kairox-dev-workflow, test-driven-rust"
    );
    await expect(page.getByTestId("agent-form-nicknames")).toHaveValue("Reviewer, Audit");
    await expect(page.getByTestId("agent-form-instructions")).toHaveValue(
      "Lead with concrete findings.\nCall out missing tests."
    );
    await page.getByTestId("agent-cancel").click();
  });

  test("copies a built-in agent into user scope", async ({ page }) => {
    await navigateToAgents(page);

    await page.getByTestId("agent-copy-worker").click();

    const workerRows = page.locator('[data-test="agent-row-worker"]');
    await expect(workerRows).toHaveCount(2);
    await expect(workerRows.nth(0)).toContainText("Built-in");
    await expect(workerRows.nth(1)).toContainText("User");
    await expect(agentRow(page, "worker", "Builtin")).toContainText("Shadowed by User:worker");
    await expect(agentRow(page, "worker", "User")).toContainText("Effective");
  });

  test("creates, verifies, and deletes a project-scoped agent", async ({ page }) => {
    await navigateToAgents(page);

    await page.getByTestId("source-btn-project").click();
    await expect(page.getByTestId("source-btn-project")).toHaveClass(/active/);

    await page.getByTestId("agent-new").click();
    await expect(page.getByTestId("agent-editor-dialog")).toContainText("Project");

    await page.getByTestId("agent-form-name").fill("pilot-workflow-agent");
    await page.getByTestId("agent-form-description").fill("Project scoped workflow agent.");
    await page.getByTestId("agent-form-model").fill("fast");
    await page.getByTestId("agent-form-permission").fill("read_only");
    await page.getByTestId("agent-form-tools").fill("fs.read, shell");
    await page.getByTestId("agent-form-skills").fill("kairox-dev-workflow");
    await page.getByTestId("agent-form-nicknames").fill("Pilot");
    await page.getByTestId("agent-form-enabled").click();
    await expect(page.getByTestId("agent-form-enabled")).not.toBeChecked();
    await page
      .getByTestId("agent-form-instructions")
      .fill("Validate the real desktop settings workflow.");
    await page.getByTestId("agent-save").click();

    const projectRow = agentRow(page, "pilot-workflow-agent", "Project");
    await expect(projectRow).toBeVisible();
    await expect(projectRow).toContainText("Project");
    await expect(projectRow).toContainText("Disabled");
    await expect(projectRow).toContainText("fast");
    await expect(projectRow).toContainText("read_only");
    await expect(projectRow).toContainText("fs.read, shell");

    await projectRow.getByTestId("agent-edit-pilot-workflow-agent").click();
    await expect(page.getByTestId("agent-form-name")).toHaveValue("pilot-workflow-agent");
    await expect(page.getByTestId("agent-form-enabled")).not.toBeChecked();
    await expect(page.getByTestId("agent-form-instructions")).toHaveValue(
      "Validate the real desktop settings workflow."
    );
    await page.getByTestId("agent-cancel").click();

    await projectRow.getByTestId("agent-delete-pilot-workflow-agent").click();
    await expect(projectRow).toHaveCount(0);
  });
});
