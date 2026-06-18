import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

import {
  checkReleaseDocs,
  deriveReleaseFields,
  readWorkspaceVersion,
  syncReleaseDocs
} from "./release-version-docs.mjs";

async function writeFixture(root, path, content) {
  const full = join(root, path);
  await mkdir(dirname(full), { recursive: true });
  await writeFile(full, content);
}

async function readFixture(root, path) {
  return readFile(join(root, path), "utf8");
}

async function createFixtureRepo() {
  const root = await mkdtemp(join(tmpdir(), "kairox-release-docs-"));

  await writeFixture(
    root,
    "Cargo.toml",
    `[workspace]
members = []

[workspace.package]
edition = "2021"
version = "0.41.0"

[workspace.dependencies]
some-crate = "9.9.9"
`
  );
  await writeFixture(
    root,
    "docs/current-release.json",
    `{
  "version": "0.37.0",
  "releaseDate": "2026-06-18"
}
`
  );
  await writeFixture(
    root,
    "README.md",
    "Kairox is in active development (current release `v0.37.0`) with a fully interactive TUI.\n"
  );
  await writeFixture(
    root,
    "ROADMAP.md",
    `# Roadmap

## Near term

- ✅ Existing item
`
  );
  await writeFixture(
    root,
    "docs/ROADMAP.md",
    `# Kairox Roadmap

> Living document. Updated as milestones ship or priorities shift.
> Current version: **0.37.0** (2026-06-05).

| Memory + context assembly                 | ✅ Multi-scope memory + tiktoken budgets + compaction | Competitive; RAG retrieval is gap                                    |

## Phase 4 — Knowledge and retrieval (v0.43+)
`
  );
  await writeFixture(
    root,
    "site/community/roadmap.md",
    `## What ships today (v0.38.x)

### Memory and context

- Tiktoken-based context budgeting with auto-compaction at a configurable threshold.
`
  );
  await writeFixture(
    root,
    "site/zh/community/roadmap.md",
    `## 当前已发布（v0.38.x）

### Memory 与上下文

- 基于 tiktoken 的上下文 budget 控制，达到可配置阈值时自动 compaction。
`
  );
  await writeFixture(
    root,
    "site/concepts/extensibility.md",
    `"compatibility": {
    "kairoxVersion": ">=0.37.0 <0.38.0"
  }
`
  );
  await writeFixture(
    root,
    "site/zh/concepts/extensibility.md",
    `"compatibility": {
    "kairoxVersion": ">=0.37.0 <0.38.0"
  }
`
  );

  return root;
}

test("reads workspace package version instead of dependency versions", async () => {
  const root = await createFixtureRepo();

  assert.equal(await readWorkspaceVersion(root), "0.41.0");
});

test("reads workspace package version when the package section is last", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-release-docs-last-section-"));
  await writeFixture(
    root,
    "Cargo.toml",
    `[workspace]
members = []

[workspace.package]
edition = "2021"
version = "0.41.1"
`
  );

  assert.equal(await readWorkspaceVersion(root), "0.41.1");
});

test("derives display version, minor line, and plugin compatibility range", () => {
  assert.deepEqual(deriveReleaseFields("0.41.0", "2026-06-18"), {
    version: "0.41.0",
    releaseDate: "2026-06-18",
    displayVersion: "v0.41.0",
    minorLine: "v0.41.x",
    compatRange: ">=0.41.0 <0.42.0"
  });
});

test("syncReleaseDocs rewrites stale release state across docs and site examples", async () => {
  const root = await createFixtureRepo();

  const result = await syncReleaseDocs(root, { write: true, today: "2026-06-18" });

  assert.deepEqual(result.changedPaths.sort(), [
    "README.md",
    "ROADMAP.md",
    "docs/ROADMAP.md",
    "docs/current-release.json",
    "site/community/roadmap.md",
    "site/concepts/extensibility.md",
    "site/zh/community/roadmap.md",
    "site/zh/concepts/extensibility.md"
  ]);
  assert.match(await readFixture(root, "README.md"), /current release `v0\.41\.0`/);
  assert.match(await readFixture(root, "ROADMAP.md"), /\*\*0\.41\.0\*\* \(2026-06-18\)/);
  assert.match(await readFixture(root, "docs/ROADMAP.md"), /RAG\/KB retrieval/);
  assert.match(
    await readFixture(root, "docs/ROADMAP.md"),
    /Knowledge and retrieval \(v0\.41\+\) ✅/
  );
  assert.match(
    await readFixture(root, "site/community/roadmap.md"),
    /What ships today \(v0\.41\.x\)/
  );
  assert.match(
    await readFixture(root, "site/community/roadmap.md"),
    /Workspace RAG with `WorkspaceRagIndex`/
  );
  assert.match(await readFixture(root, "site/zh/community/roadmap.md"), /当前已发布（v0\.41\.x）/);
  assert.match(await readFixture(root, "site/concepts/extensibility.md"), />=0\.41\.0 <0\.42\.0/);
  assert.match(await readFixture(root, "docs/current-release.json"), /"version": "0\.41\.0"/);
  assert.match(await readFixture(root, "docs/current-release.json"), /"releaseDate": "2026-06-18"/);
});

test("syncReleaseDocs preserves release date when the release version already matches", async () => {
  const root = await createFixtureRepo();
  await writeFixture(
    root,
    "docs/current-release.json",
    `{
  "version": "0.41.0",
  "releaseDate": "2026-06-05"
}
`
  );

  await syncReleaseDocs(root, { write: true, today: "2026-06-18" });

  assert.match(await readFixture(root, "docs/current-release.json"), /"version": "0\.41\.0"/);
  assert.match(await readFixture(root, "docs/current-release.json"), /"releaseDate": "2026-06-05"/);
});

test("checkReleaseDocs reports stale docs without writing files", async () => {
  const root = await createFixtureRepo();

  const result = await checkReleaseDocs(root);

  assert.equal(result.ok, false);
  assert(result.changedPaths.includes("README.md"));
  assert.match(await readFixture(root, "README.md"), /v0\.37\.0/);
});
