import { defineConfig, devices } from "@playwright/test";
import { DEFAULT_DEV_PORT, parsePort } from "./scripts/dev-port.mjs";

const devPort = parsePort(process.env.KAIROX_DEV_PORT, DEFAULT_DEV_PORT);
const devBaseUrl = `http://localhost:${devPort}`;

/**
 * Playwright E2E configuration for Kairox GUI.
 *
 * Strategy:
 *   1. Launch the Vite dev server (Vue SPA without Tauri)
 *   2. Inject a Tauri IPC mock that simulates the Rust backend
 *   3. Test all frontend UI flows: sessions, chat, trace, permissions, memory
 *
 * For full-stack Tauri testing (with real Rust backend), use tauri-driver + WDIO.
 * See: https://tauri.app/develop/testing/webdriver/
 */
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: "html",
  timeout: 30_000,
  expect: { timeout: 10_000 },

  use: {
    baseURL: devBaseUrl,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    // Project convention is `data-test` for component test hooks (matches the
    // attribute used in stores/mcp.ts, marketplace components, etc.).
    testIdAttribute: "data-test"
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] }
    }
  ],

  webServer: {
    command: "bun run dev",
    port: devPort,
    env: {
      KAIROX_DEV_PORT: String(devPort),
      KAIROX_DEV_STRICT_PORT: "1"
    },
    reuseExistingServer: !process.env.CI,
    timeout: 30_000
  }
});
