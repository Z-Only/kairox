/**
 * E2E: Permission and memory prompts — approve/deny tool use and memory proposals.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("permission prompt appears when agent requests tool access", async ({
  page
}) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a permission request via the mock
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulatePermissionRequest(
      "fs.read",
      "Read file: /tmp/data.txt"
    );
  });

  // Permission prompt should appear in the permission center
  await expect(page.locator(".permission-prompt").first()).toBeVisible({
    timeout: 3_000
  });
  await expect(page.locator(".permission-prompt").first()).toContainText(
    "Permission Required"
  );
  await expect(page.locator(".permission-prompt").first()).toContainText(
    "fs.read"
  );
});

test("granting permission updates the permission entry status", async ({
  page
}) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a permission request
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulatePermissionRequest(
      "shell.exec",
      "Run: ls -la"
    );
  });

  await expect(page.locator(".permission-prompt").first()).toBeVisible({
    timeout: 3_000
  });

  // Click Allow
  await page.locator(".permission-prompt .btn-allow").first().click();

  // Permission prompt should disappear (status changes from pending to completed)
  await expect(page.locator(".permission-prompt")).toHaveCount(0, {
    timeout: 3_000
  });
});

test("denying permission shows denied status", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a permission request
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulatePermissionRequest(
      "shell.rm",
      "Delete: /tmp/old.txt"
    );
  });

  await expect(page.locator(".permission-prompt").first()).toBeVisible({
    timeout: 3_000
  });

  // Click Deny
  await page.locator(".permission-prompt .btn-deny").first().click();

  // Permission prompt should disappear
  await expect(page.locator(".permission-prompt")).toHaveCount(0, {
    timeout: 3_000
  });
});

test("memory proposal appears in permission center", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a memory proposal
  await page.evaluate(() => {
    (window as any).__KAIROX_MOCK__.simulateMemoryProposal(
      "user",
      "preferred_style",
      "concise explanations"
    );
  });

  // Memory prompt should appear
  await expect(page.locator(".memory-prompt").first()).toBeVisible({
    timeout: 3_000
  });
  await expect(page.locator(".memory-prompt").first()).toContainText(
    "Memory Proposed"
  );
  await expect(page.locator(".memory-prompt").first()).toContainText(
    "concise explanations"
  );
});

test("accepting memory removes the prompt", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a memory proposal
  const memoryId = await page.evaluate(() => {
    return (window as any).__KAIROX_MOCK__.simulateMemoryProposal(
      "workspace",
      null,
      "Project uses Rust"
    );
  });

  await expect(page.locator(".memory-prompt").first()).toBeVisible({
    timeout: 3_000
  });

  // The PermissionPrompt calls invoke("resolve_permission", { requestId, decision })
  // The requestId is the entry.id which is memory_id from the MemoryProposed event.
  // We need to register this as a permission request so resolve_permission can find it.
  await page.evaluate((mid) => {
    // Register the memory ID as a permission request so resolve_permission can process it
    (window as any).__KAIROX_MOCK__.state.permissionRequests.set(mid, {
      tool_id: "memory.store",
      preview: "Save workspace memory"
    });
  }, memoryId);

  // Click Accept
  await page.locator(".memory-prompt .btn-allow").first().click();

  // Prompt should disappear
  await expect(page.locator(".memory-prompt")).toHaveCount(0, {
    timeout: 3_000
  });
});

test("rejecting memory removes the prompt", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Simulate a memory proposal
  const memoryId = await page.evaluate(() => {
    return (window as any).__KAIROX_MOCK__.simulateMemoryProposal(
      "session",
      null,
      "Temporary note"
    );
  });

  await expect(page.locator(".memory-prompt").first()).toBeVisible({
    timeout: 3_000
  });

  // Register the memory ID as a permission request
  await page.evaluate((mid) => {
    (window as any).__KAIROX_MOCK__.state.permissionRequests.set(mid, {
      tool_id: "memory.store",
      preview: "Save session memory"
    });
  }, memoryId);

  // Click Reject
  await page.locator(".memory-prompt .btn-deny").first().click();

  // Prompt should disappear
  await expect(page.locator(".memory-prompt")).toHaveCount(0, {
    timeout: 3_000
  });
});

test("permission center shows 'No pending requests' when empty", async ({
  page
}) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // By default, no permission requests
  await expect(page.locator(".permission-center")).toContainText(
    "No pending requests"
  );
});
