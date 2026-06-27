import { access } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function inferMainCheckout(repoRoot) {
  const marker = "/.worktrees/";
  const index = repoRoot.indexOf(marker);
  if (index === -1) {
    return null;
  }
  return repoRoot.slice(0, index);
}

function candidateRoots(repoRoot, mainCheckout) {
  const configuredMainCheckout = mainCheckout ?? process.env.KAIROX_MAIN_CHECKOUT;
  return [configuredMainCheckout, inferMainCheckout(repoRoot), repoRoot].filter(Boolean);
}

async function hasPlaywrightPackage(nodePath) {
  try {
    await access(join(nodePath, "playwright", "package.json"));
    return true;
  } catch {
    return false;
  }
}

export async function resolvePlaywrightNodePath({
  repoRoot = process.cwd(),
  mainCheckout = null
} = {}) {
  const seen = new Set();
  for (const root of candidateRoots(resolve(repoRoot), mainCheckout && resolve(mainCheckout))) {
    for (const nodePath of [
      join(root, "node_modules", ".bun", "node_modules"),
      join(root, "node_modules")
    ]) {
      if (seen.has(nodePath)) {
        continue;
      }
      seen.add(nodePath);
      if (await hasPlaywrightPackage(nodePath)) {
        return nodePath;
      }
    }
  }
  return null;
}

function escapeDoubleQuoted(value) {
  return value.replace(/["\\$`]/g, "\\$&");
}

export function formatNodePathExport(nodePath) {
  return `PW_NODE_PATH="${escapeDoubleQuoted(nodePath)}"`;
}

async function main() {
  const nodePath = await resolvePlaywrightNodePath();
  if (!nodePath) {
    console.error(
      "Unable to find Playwright. Looked for playwright/package.json under main checkout and current worktree node_modules."
    );
    process.exit(1);
  }

  console.log(process.argv.includes("--export") ? formatNodePathExport(nodePath) : nodePath);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
