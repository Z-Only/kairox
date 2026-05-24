/**
 * E2E: MCP Marketplace — browse, filter, install (happy + runtime-missing),
 * uninstall flows backed by tauri-mock fixtures.
 *
 * Marketplace is accessed through the Settings page's MCP tab.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./helpers/tauriMock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/");
  await page.getByTestId("nav-settings").click();
  await Promise.all([
    page.waitForURL(/#\/settings\/mcp$/),
    page.getByTestId("settings-tab-mcp").click()
  ]);
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
    await page.getByTestId("config-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    // Wait for the install to complete (progress text shows "Install complete.").
    await expect(page.getByTestId("install-progress")).toContainText(/complete/i, {
      timeout: 10_000
    });
    await page.getByTestId("install-close").click();
    await expect(page.getByTestId("catalog-installed-status")).toContainText("Installed");
    await page.getByTestId("catalog-test-connectivity").click();
    await expect(page.getByTestId("catalog-connectivity-result")).toContainText(
      "Connected (1 tools)"
    );
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
    await page.getByTestId("config-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toContainText("Missing runtimes");
    await expect(page.getByTestId("install-progress")).toContainText("npx");
  });

  test("keeps installed server management out of the marketplace tabs", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("config-WORKSPACE_PATH").fill("/tmp/demo");
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

  test("filters remote catalog sources by search", async ({ page }) => {
    await page.getByTestId("catalog-source-settings").click();
    await expect(page.getByTestId("catalog-source-settings-drawer")).toBeVisible();
    await expect(page.getByTestId("catalog-source-search-input")).toHaveCount(0);

    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("smithery");
    await page.getByTestId("src-name").fill("Smithery");
    await page.getByTestId("src-url").fill("https://registry.smithery.ai");
    await page.getByTestId("src-save").click();

    await page.getByTestId("add-source-toggle").click();
    await page.getByTestId("src-id").fill("team-registry");
    await page.getByTestId("src-name").fill("Team Registry");
    await page.getByTestId("src-url").fill("https://registry.internal.example");
    await page.getByTestId("src-save").click();

    await page.getByTestId("catalog-source-search-input").fill("smithery");
    await expect(page.getByTestId("catalog-source-row-smithery")).toBeVisible();
    await expect(page.getByTestId("catalog-source-row-team-registry")).toHaveCount(0);

    await page.getByTestId("catalog-source-search-input").fill("internal");
    await expect(page.getByTestId("catalog-source-row-team-registry")).toBeVisible();
    await expect(page.getByTestId("catalog-source-row-smithery")).toHaveCount(0);

    await page.getByTestId("catalog-source-search-input").fill("does-not-exist");
    await expect(page.getByTestId("catalog-sources-filter-empty")).toContainText(
      "No remote catalog sources match your search."
    );
    await expect(page.getByTestId("catalog-sources-list")).toHaveCount(0);
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
    await expect(remote).toHaveClass(/kx-chip-button--selected/);

    // Deselect the remote chip → only builtin entries remain.
    await remote.click();
    await expect(remote).not.toHaveClass(/kx-chip-button--selected/);
    // Builtin chip stays active and filesystem card is still visible.
    await expect(page.getByTestId("source-chip-builtin")).toHaveClass(/kx-chip-button--selected/);
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
    await expect(page.getByTestId("skill-audit-project-project-review")).toContainText("Source");
    await expect(page.getByTestId("skill-audit-project-project-review")).toContainText("Active");
    await expect(page.getByTestId("skill-invalid-project-invalid-workspace-skill")).toContainText(
      "Missing required description"
    );

    await expect(page.getByTestId("skill-installed-search-input")).toBeVisible();
    await page.getByTestId("skill-installed-search-input").fill("registry");
    await expect(page.getByTestId("skill-row-project-registry-review")).toBeVisible();
    await expect(page.getByTestId("skill-row-project-project-review")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("invalid");
    await expect(page.getByTestId("skill-row-project-invalid-workspace-skill")).toBeVisible();
    await expect(page.getByTestId("skill-row-project-registry-review")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("does-not-exist");
    await expect(page.getByTestId("skill-installed-filter-empty")).toContainText(
      "No installed skills match your search."
    );
    await expect(page.getByTestId("skill-installed-list")).toHaveCount(0);

    await page.getByTestId("skill-installed-search-input").fill("");
    await expect(page.getByTestId("skill-row-project-project-review")).toBeVisible();

    await page.getByTestId("skill-enabled-project-project-review").click();
    await expect(page.getByTestId("skill-row-project-project-review")).toContainText("Disabled");

    await page.getByTestId("skill-update-project-registry-review").click();
    await expect(page.getByTestId("skill-row-project-registry-review")).toContainText("up to date");

    // Switch to Discover sub-tab to search remote skills
    await page.getByTestId("skill-subtab-discover").click();
    await expect(page.getByTestId("skill-catalog-refresh")).toBeVisible();
    await page.getByTestId("skill-catalog-refresh").click();
    await expect(page.getByTestId("skill-source-filter-skillhub")).toBeVisible();
    await page.getByTestId("skill-source-filter-skillhub").click();
    await page.getByTestId("skill-catalog-search").fill("review");
    await page.getByTestId("skill-catalog-search").press("Enter");
    await expect(page.getByTestId("skill-catalog-card")).toBeVisible();
    await page.getByTestId("skill-catalog-install-skillhub/code-review-assistant").click();
    await expect(page.getByTestId("skill-catalog-install-success")).toContainText(
      "Installed Code Review Assistant"
    );
    await expect(
      page.getByTestId("skill-catalog-install-skillhub/code-review-assistant")
    ).toHaveText("Installed");
    // Switch back to Installed tab. Marketplace installs follow the top-level
    // Settings source selection, which is still Project from the setup above.
    await page.getByTestId("skill-subtab-installed").click();
    await expect(page.getByTestId("skill-row-project-code-review-assistant")).toContainText(
      "Code Review Assistant"
    );

    await page.getByTestId("skill-delete-project-code-review-assistant").click();
    await expect(page.getByTestId("skill-row-project-code-review-assistant")).toHaveCount(0);
  });

  test("shows effective audit state on model profile settings", async ({ page }) => {
    await page.getByTestId("settings-tab-models").click();

    await expect(page.getByTestId("model-audit-fast")).toContainText("Source");
    await expect(page.getByTestId("model-audit-fast")).toContainText("Enabled");
    await expect(page.getByTestId("model-audit-fast")).toContainText("Active");
  });

  test("adds a remote skill catalog source from the discover drawer", async ({ page }) => {
    await page.getByTestId("settings-tab-skills").click();
    await page.getByTestId("skill-subtab-discover").click();

    await page.getByTestId("skill-source-settings-btn").click();
    await expect(page.getByTestId("skill-source-settings-drawer")).toBeVisible();

    await page.getByTestId("skill-add-source-toggle").click();
    await page.getByTestId("skill-src-id").fill("custom-skillhub");
    await page.getByTestId("skill-src-name").fill("Custom SkillHub");
    await page.getByTestId("skill-src-url").fill("https://api.skillhub.example");
    await page.getByTestId("skill-src-save").click();

    await expect(page.getByTestId("skill-source-filter-custom-skillhub")).toBeVisible();
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
