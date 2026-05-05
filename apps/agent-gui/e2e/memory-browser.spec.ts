/**
 * E2E: Memory browser — view, filter, and delete memories.
 */
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

test.beforeEach(async ({ page }) => {
  const mockPath = resolve(__dirname, "tauri-mock.js");
  await page.addInitScript({ path: mockPath });
});

test("memory browser tab is accessible from trace timeline", async ({
  page
}) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Click the Memory tab
  await page.locator(".tab-group >> button", { hasText: "Memory" }).click();

  // Memory browser should now be visible
  await expect(page.locator(".memory-browser")).toBeVisible();
  await expect(page.locator(".memory-header")).toContainText("Memories");
});

test("memory browser shows empty state when no memories", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Navigate to memory tab
  await page.locator(".tab-group >> button", { hasText: "Memory" }).click();
  await expect(page.locator(".memory-browser")).toBeVisible();

  // Should show empty state
  await expect(page.locator(".memory-empty")).toContainText("No memories");
});

test("memory browser displays memories from mock state", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Add memories through the mock
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    mock.state.memories = [
      {
        id: "mem_1",
        scope: "user",
        key: "style",
        content: "concise",
        accepted: true
      },
      {
        id: "mem_2",
        scope: "workspace",
        key: "lang",
        content: "Rust",
        accepted: true
      },
      {
        id: "mem_3",
        scope: "session",
        key: null,
        content: "Temporary note",
        accepted: false
      }
    ];
  });

  // Navigate to memory tab and trigger refresh
  await page.locator(".tab-group >> button", { hasText: "Memory" }).click();
  await expect(page.locator(".memory-browser")).toBeVisible();

  // Click refresh button to reload memories from mock
  await page.locator(".refresh-btn").click();

  // Should show the 3 memories
  await expect(page.locator(".memory-item")).toHaveCount(3, { timeout: 3000 });
});

test("query_memories mock returns correct data", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Add memories through the mock
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    mock.state.memories = [
      {
        id: "mem_1",
        scope: "user",
        key: "style",
        content: "concise",
        accepted: true
      },
      {
        id: "mem_2",
        scope: "workspace",
        key: "lang",
        content: "Rust",
        accepted: true
      },
      {
        id: "mem_3",
        scope: "session",
        key: null,
        content: "Temporary note",
        accepted: false
      }
    ];
  });

  // Query all memories
  const memories = await page.evaluate(async () => {
    return await (window as any).__TAURI_INTERNALS__.invoke(
      "query_memories",
      {}
    );
  });

  expect(memories).toHaveLength(3);
  expect(memories[0].scope).toBe("user");
  expect(memories[1].scope).toBe("workspace");
  expect(memories[2].scope).toBe("session");
});

test("delete_memory removes the memory from mock state", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sessions-sidebar")).toBeVisible({
    timeout: 10_000
  });

  // Add memories
  await page.evaluate(() => {
    const mock = (window as any).__KAIROX_MOCK__;
    mock.state.memories = [
      {
        id: "mem_1",
        scope: "user",
        key: "style",
        content: "concise",
        accepted: true
      },
      {
        id: "mem_2",
        scope: "workspace",
        key: "lang",
        content: "Rust",
        accepted: true
      }
    ];
  });

  // Delete one
  await page.evaluate(async () => {
    await (window as any).__TAURI_INTERNALS__.invoke("delete_memory", {
      id: "mem_1"
    });
  });

  // Verify it's gone
  const memories = await page.evaluate(async () => {
    return await (window as any).__TAURI_INTERNALS__.invoke(
      "query_memories",
      {}
    );
  });

  expect(memories).toHaveLength(1);
  expect(memories[0].id).toBe("mem_2");
});
