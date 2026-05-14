/**
 * E2E: MCP Marketplace — browse, filter, install (happy + runtime-missing),
 * uninstall flows backed by tauri-mock fixtures.
 *
 * Marketplace is accessed through the Settings page's MCP tab.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
  await page.goto("/");
  await page.getByTestId("nav-settings").click();
  await page.getByTestId("settings-tab-mcp").click();
  await page.getByTestId("mcp-subtab-marketplace").click();
});

test.describe("Marketplace", () => {
  test("browses the catalog and shows the filesystem entry", async ({ page }) => {
    const card = page.getByTestId("catalog-card").filter({ hasText: "Filesystem" });
    await expect(card).toBeVisible();
  });

  test("filters by keyword", async ({ page }) => {
    await page.getByTestId("catalog-search").fill("filesystem");
    await expect(page.getByTestId("catalog-card")).toHaveCount(1);
  });

  test("installs the filesystem entry happy path", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    // Wait for the install to complete (progress text shows "Install complete.").
    await expect(page.getByTestId("install-progress")).toContainText(/complete/i, {
      timeout: 10_000
    });
    await page.getByTestId("install-close").click();
    // Close the CatalogDetail drawer that still overlays the page.
    await page.locator(".drawer-close-btn").click();
    await expect(page.getByTestId("tab-installed")).toHaveCount(0);
    await expect(page.getByTestId("catalog-card").filter({ hasText: "Filesystem" })).toBeVisible();
  });

  test("runtime-missing path shows a missing npx hint", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error injected on window for tauri-mock to read
      window.__MARKETPLACE_FORCE_MISSING__ = ["npx"];
    });
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toContainText("Missing runtimes");
    await expect(page.getByTestId("install-progress")).toContainText("npx");
  });

  test("keeps installed server management out of the marketplace tabs", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    // Wait for the install to complete before closing.
    await expect(page.getByTestId("install-progress")).toContainText(/complete/i, {
      timeout: 10_000
    });
    await page.getByTestId("install-close").click();
    // Close the CatalogDetail drawer that still overlays the page.
    await page.locator(".drawer-close-btn").click();

    await expect(page.getByTestId("tab-installed")).toHaveCount(0);
    await page.getByTestId("mcp-subtab-installed").click();
    await expect(page.getByTestId("mcp-installed-servers")).toBeVisible();
  });
});

test.describe("Marketplace — Phase 2 remote catalog sources", () => {
  test("user can add and remove a remote catalog source", async ({ page }) => {
    // Open the source-settings drawer.
    await page.getByTestId("catalog-source-settings").click();
    await expect(page.getByTestId("catalog-source-settings-drawer")).toBeVisible();
    await expect(page.getByText("No remote catalog sources")).toBeVisible();

    // Add a new source.
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    // The new chip appears in the marketplace toolbar.
    await expect(page.getByTestId("source-chip-smithery")).toBeVisible();

    // Remove it via the drawer.
    await page.getByTestId("src-remove-smithery").click();
    await expect(page.getByTestId("source-chip-smithery")).toHaveCount(0);
  });

  test("toggling source chip filters card grid", async ({ page }) => {
    // Add a remote source so we have a non-builtin chip to toggle.
    await page.getByTestId("catalog-source-settings").click();
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    // Close the settings modal so it doesn't block chip interaction.
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("catalog-source-settings-drawer")).not.toBeVisible();

    // The new remote chip starts active.
    const remote = page.getByTestId("source-chip-smithery");
    await expect(remote).toBeVisible();
    await expect(remote).toHaveClass(/active/);

    // Deselect the remote chip → only builtin entries remain.
    await remote.click();
    await expect(remote).not.toHaveClass(/active/);
    // Builtin chip stays active and filesystem card is still visible.
    await expect(page.getByTestId("source-chip-builtin")).toHaveClass(/active/);
    await expect(page.getByTestId("catalog-card").filter({ hasText: "Filesystem" })).toBeVisible();
  });

  test("validates URL when adding a source", async ({ page }) => {
    await page.getByTestId("catalog-source-settings").click();
    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("bad");
    await page.getByTestId("src-name").fill("Bad");
    await page.getByTestId("src-url").fill("not-a-url");
    await page.getByTestId("src-save").click();
    await expect(page.getByText(/url must start with http/i)).toBeVisible();
    // The chip should NOT have been created.
    await expect(page.getByTestId("source-chip-bad")).toHaveCount(0);
  });
});

test.describe("Settings panes backed by tauri-mock", () => {
  test("manages MCP settings server state", async ({ page }) => {
    await page.getByTestId("mcp-subtab-installed").click();
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("GitHub");
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("Enabled");

    await page.getByTestId("mcp-enable-github").click();
    await expect(page.getByTestId("mcp-server-row-github")).toContainText("Disabled");

    // Add server via icon-button → dropdown → manual dialog
    await page.getByTestId("mcp-add-server-btn").click();
    await page.getByTestId("mcp-add-server-manual").click();
    await page.getByTestId("mcp-form-name").fill("Local Tools");
    await page.getByTestId("mcp-form-command").fill("node");
    await page.getByTestId("mcp-form-args").fill("server.js --stdio");
    await page.getByTestId("mcp-save-button").click();
    await expect(page.getByTestId("mcp-server-row-local-tools")).toContainText("Local Tools");

    await page.getByTestId("mcp-delete-local-tools").click();
    await expect(page.getByTestId("mcp-server-row-local-tools")).toHaveCount(0);
  });

  test("manages skill settings discovery, install, update, and delete", async ({ page }) => {
    await page.getByTestId("settings-tab-skills").click();

    // Switch to project config source to see project-scoped skills
    await page.getByTestId("source-btn-project").click();
    await expect(page.getByTestId("skill-row-project-project-review")).toContainText(
      "Project Review"
    );
    await expect(page.getByTestId("skill-invalid-project-invalid-workspace-skill")).toContainText(
      "Missing required description"
    );

    await page.getByTestId("skill-enabled-project-project-review").click();
    await expect(page.getByTestId("skill-row-project-project-review")).toContainText("Disabled");

    await page.getByTestId("skill-update-project-registry-review").click();
    await expect(page.getByTestId("skill-row-project-registry-review")).toContainText("up to date");

    // Switch to Discover sub-tab to search remote skills
    await page.getByTestId("skill-subtab-discover").click();
    await page.getByTestId("skill-catalog-search").fill("review");
    await page.getByTestId("skill-catalog-search").press("Enter");
    await expect(page.getByTestId("skill-catalog-card")).toBeVisible();
    await page.getByTestId("skill-catalog-install-skillhub/code-review-assistant").click();
    // Switch back to Installed tab, then to User source to verify the installed user-scoped skill
    await page.getByTestId("skill-subtab-installed").click();
    await page.getByTestId("source-btn-user").click();
    await expect(page.getByTestId("skill-row-user-code-review-assistant")).toContainText(
      "Code Review Assistant"
    );

    await page.getByTestId("skill-delete-user-code-review-assistant").click();
    await expect(page.getByTestId("skill-row-user-code-review-assistant")).toHaveCount(0);
  });

  test("mock rejects ambiguous legacy skill ids without mutating rows", async ({ page }) => {
    await page.getByTestId("settings-tab-skills").click();

    const ambiguityResult = await page.evaluate(async () => {
      const mockWindow = window as unknown as {
        __KAIROX_MOCK__: {
          state: {
            skillSettings: Array<{
              settings_id: string;
              id: string;
              name: string;
              enabled: boolean;
            }>;
          };
        };
        __TAURI_INTERNALS__: {
          invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
        };
      };

      const projectReview = mockWindow.__KAIROX_MOCK__.state.skillSettings.find(
        (skill) => skill.settings_id === "project:project-review"
      );
      if (!projectReview) {
        throw new Error("missing project review fixture");
      }

      mockWindow.__KAIROX_MOCK__.state.skillSettings.push({
        ...projectReview,
        settings_id: "user:project-review",
        name: "User Project Review",
        enabled: true
      });

      const captureRejection = async (
        operation: () => Promise<unknown>
      ): Promise<string | null> => {
        try {
          await operation();
          return null;
        } catch (error) {
          return error instanceof Error ? error.message : String(error);
        }
      };

      const enableError = await captureRejection(() =>
        mockWindow.__TAURI_INTERNALS__.invoke("set_skill_enabled", {
          skillId: "project-review",
          enabled: false
        })
      );
      const deleteError = await captureRejection(() =>
        mockWindow.__TAURI_INTERNALS__.invoke("delete_skill_settings", {
          skillId: "project-review"
        })
      );
      const reviewRows = mockWindow.__KAIROX_MOCK__.state.skillSettings.filter(
        (skill) => skill.id === "project-review"
      );

      return {
        enableError,
        deleteError,
        rowCount: reviewRows.length,
        enabledStates: reviewRows.map((skill) => skill.enabled)
      };
    });

    expect(ambiguityResult.enableError).toContain("ambiguous skill id");
    expect(ambiguityResult.deleteError).toContain("ambiguous skill id");
    expect(ambiguityResult.rowCount).toBe(2);
    expect(ambiguityResult.enabledStates).toEqual([true, true]);
  });
});
