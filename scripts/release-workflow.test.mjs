import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const workflowPath = join(repoRoot, ".github/workflows/release-build.yml");

async function readWorkflow() {
  return readFile(workflowPath, "utf8");
}

test("release workflow uses the tauri-action v1 asset naming input", async () => {
  const workflow = await readWorkflow();

  assert.match(workflow, /uses: tauri-apps\/tauri-action@v1/);
  assert.match(
    workflow,
    /^\s+releaseAssetNamePattern: Kairox_\$\{\{ github\.ref_name \}\}_\[platform\]_\[arch\]\[ext\]$/m
  );
  assert.doesNotMatch(workflow, /^\s+assetNamePattern:/m);
});

test("tauri builds preserve the generated release notes", async () => {
  const workflow = await readWorkflow();

  assert.match(workflow, /^\s+outputs:\n\s+body: \$\{\{ steps\.notes\.outputs\.body \}\}$/m);
  assert.match(workflow, /^\s+releaseBody: \$\{\{ needs\.publish-release\.outputs\.body \}\}$/m);
  assert.doesNotMatch(workflow, /See the assets to download this version and install\./);
});

test("tauri-action v1 is the single owner of updater metadata", async () => {
  const workflow = await readWorkflow();
  const tauriAction = workflow.match(
    /uses: tauri-apps\/tauri-action@v1[\s\S]*?(?=\n  checksums:)/
  )?.[0];

  assert.ok(tauriAction);
  assert.match(tauriAction, /^\s+uploadUpdaterJson: true$/m);
  assert.match(tauriAction, /^\s+updaterJsonPreferNsis: true$/m);
  assert.doesNotMatch(workflow, /^\s+publish-updater-json:$/m);
  assert.doesNotMatch(workflow, /Generate latest\.json for Tauri updater/);
});
