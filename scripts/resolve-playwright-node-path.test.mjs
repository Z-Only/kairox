import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  formatNodePathExport,
  resolvePlaywrightNodePath
} from "./resolve-playwright-node-path.mjs";

async function createPlaywrightInstall(root) {
  const nodePath = join(root, "node_modules", ".bun", "node_modules");
  const pkgDir = join(nodePath, "playwright");
  await mkdir(pkgDir, { recursive: true });
  await writeFile(join(pkgDir, "package.json"), '{"name":"playwright"}');
  return nodePath;
}

test("resolvePlaywrightNodePath prefers main checkout Bun install for worktrees", async () => {
  const tmp = await mkdtemp(join(tmpdir(), "kairox-pw-node-path-"));
  const worktree = join(tmp, "worktree");
  const mainCheckout = join(tmp, "main");
  await mkdir(worktree, { recursive: true });
  const expected = await createPlaywrightInstall(mainCheckout);

  assert.equal(
    await resolvePlaywrightNodePath({
      repoRoot: worktree,
      mainCheckout
    }),
    expected
  );
});

test("resolvePlaywrightNodePath infers main checkout from .worktrees path", async () => {
  const tmp = await mkdtemp(join(tmpdir(), "kairox-pw-node-path-"));
  const mainCheckout = join(tmp, "repo");
  const worktree = join(mainCheckout, ".worktrees", "feature");
  await mkdir(worktree, { recursive: true });
  const expected = await createPlaywrightInstall(mainCheckout);

  assert.equal(
    await resolvePlaywrightNodePath({
      repoRoot: worktree
    }),
    expected
  );
});

test("resolvePlaywrightNodePath lets explicit main checkout override environment", async () => {
  const tmp = await mkdtemp(join(tmpdir(), "kairox-pw-node-path-"));
  const worktree = join(tmp, "worktree");
  const envCheckout = join(tmp, "env");
  const mainCheckout = join(tmp, "main");
  await mkdir(worktree, { recursive: true });
  await createPlaywrightInstall(envCheckout);
  const expected = await createPlaywrightInstall(mainCheckout);
  const previous = process.env.KAIROX_MAIN_CHECKOUT;
  process.env.KAIROX_MAIN_CHECKOUT = envCheckout;

  try {
    assert.equal(
      await resolvePlaywrightNodePath({
        repoRoot: worktree,
        mainCheckout
      }),
      expected
    );
  } finally {
    if (previous === undefined) {
      delete process.env.KAIROX_MAIN_CHECKOUT;
    } else {
      process.env.KAIROX_MAIN_CHECKOUT = previous;
    }
  }
});

test("resolvePlaywrightNodePath falls back to worktree install", async () => {
  const tmp = await mkdtemp(join(tmpdir(), "kairox-pw-node-path-"));
  const expected = await createPlaywrightInstall(tmp);

  assert.equal(
    await resolvePlaywrightNodePath({
      repoRoot: tmp,
      mainCheckout: join(tmp, "missing-main")
    }),
    expected
  );
});

test("formatNodePathExport emits shell assignment", () => {
  assert.equal(
    formatNodePathExport("/repo/node_modules/.bun/node_modules"),
    'PW_NODE_PATH="/repo/node_modules/.bun/node_modules"'
  );
});
