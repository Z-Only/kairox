import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";

const e2eDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const scriptFiles = [
  "fixtures/tauri-mock/state.js",
  "fixtures/tauri-mock/helpers.js",
  "fixtures/tauri-mock/registry.js",
  "fixtures/tauri-mock/event-commands.js",
  "fixtures/tauri-mock/workspace-commands.js",
  "fixtures/tauri-mock/profile-commands.js",
  "fixtures/tauri-mock/session-commands.js",
  "fixtures/tauri-mock/project-commands.js",
  "fixtures/tauri-mock/memory-commands.js",
  "fixtures/tauri-mock/mcp-commands.js",
  "fixtures/tauri-mock/skill-commands.js",
  "fixtures/tauri-mock/instructions-commands.js",
  "fixtures/tauri-mock/hooks-commands.js",
  "fixtures/tauri-mock/marketplace-commands.js",
  "tauri-mock.js"
];

function buildTauriMockScript(): string {
  return scriptFiles.map((file) => readFileSync(resolve(e2eDirectory, file), "utf8")).join("\n;\n");
}

export async function installTauriMock(page: Page): Promise<void> {
  await page.addInitScript({ content: buildTauriMockScript() });
}
