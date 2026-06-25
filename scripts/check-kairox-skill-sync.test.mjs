import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

import { evaluateSkillSync } from "./check-kairox-skill-sync.mjs";

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

async function writeFixture(root, path, content) {
  const full = join(root, path);
  await mkdir(dirname(full), { recursive: true });
  await writeFile(full, content);
}

async function createFixtureRepo() {
  return mkdtemp(join(tmpdir(), "kairox-skill-sync-"));
}

async function writeSkill(root, name, content) {
  await writeFixture(root, `.agents/skills/${name}/SKILL.md`, content);
}

async function writeManifest(root, skills) {
  await writeFixture(
    root,
    "docs/ai/kairox-skills/manifest.json",
    `${JSON.stringify({ version: 1, skills }, null, 2)}\n`
  );
}

test("evaluateSkillSync fails when a local skill hash does not match the manifest", async () => {
  const root = await createFixtureRepo();
  const skillName = "kairox-dev-workflow";
  const skillPath = `.agents/skills/${skillName}/SKILL.md`;
  const localContent = "# Local skill\n";
  await writeSkill(root, skillName, localContent);
  await writeManifest(root, [
    {
      name: skillName,
      path: skillPath,
      sha256: sha256("# Stale tracked hash\n"),
      updated_at: "2026-06-26"
    }
  ]);

  const result = await evaluateSkillSync({
    repoRoot: root,
    manifestPath: join(root, "docs/ai/kairox-skills/manifest.json")
  });

  assert.equal(result.ok, false);
  assert.equal(result.mismatches.length, 1);
  assert.deepEqual(
    {
      code: result.mismatches[0].code,
      name: result.mismatches[0].name,
      path: result.mismatches[0].path,
      expected: result.mismatches[0].expected,
      actual: result.mismatches[0].actual
    },
    {
      code: "hash_mismatch",
      name: skillName,
      path: skillPath,
      expected: sha256("# Stale tracked hash\n"),
      actual: sha256(localContent)
    }
  );
  assert.match(result.suggestedRefreshText, /refresh docs\/ai\/kairox-skills\/manifest\.json/i);
  assert.equal(await readFile(join(root, skillPath), "utf8"), localContent);
});

test("evaluateSkillSync passes when manifest entries match local SKILL.md files", async () => {
  const root = await createFixtureRepo();
  const skillName = "kairox-dev-workflow";
  const skillPath = `.agents/skills/${skillName}/SKILL.md`;
  const localContent = "# Synced skill\n";
  await writeSkill(root, skillName, localContent);
  await writeManifest(root, [
    {
      name: skillName,
      path: skillPath,
      sha256: sha256(localContent),
      updated_at: "2026-06-26"
    }
  ]);

  const result = await evaluateSkillSync({
    repoRoot: root,
    manifestPath: join(root, "docs/ai/kairox-skills/manifest.json")
  });

  assert.equal(result.ok, true);
  assert.deepEqual(result.invalid, []);
  assert.deepEqual(result.missing, []);
  assert.deepEqual(result.mismatches, []);
});

test("evaluateSkillSync reports missing local skills with a stable code", async () => {
  const root = await createFixtureRepo();
  const skillName = "kairox-dev-workflow";
  const skillPath = `.agents/skills/${skillName}/SKILL.md`;
  await writeManifest(root, [
    {
      name: skillName,
      path: skillPath,
      sha256: sha256("# Missing locally\n"),
      updated_at: "2026-06-26"
    }
  ]);

  const result = await evaluateSkillSync({
    repoRoot: root,
    manifestPath: join(root, "docs/ai/kairox-skills/manifest.json")
  });

  assert.equal(result.ok, false);
  assert.deepEqual(result.missing, [
    {
      code: "missing_local_skill",
      name: skillName,
      path: skillPath
    }
  ]);
});

test("evaluateSkillSync reports manifest entries with missing fields using stable codes", async () => {
  const root = await createFixtureRepo();
  await writeManifest(root, [
    {
      name: "kairox-dev-workflow",
      path: ".agents/skills/kairox-dev-workflow/SKILL.md",
      updated_at: "2026-06-26"
    }
  ]);

  const result = await evaluateSkillSync({
    repoRoot: root,
    manifestPath: join(root, "docs/ai/kairox-skills/manifest.json")
  });

  assert.equal(result.ok, false);
  assert.deepEqual(result.invalid, [
    {
      code: "invalid_manifest_entry",
      index: 0,
      field: "sha256",
      message: "skills[0].sha256 must be a sha256 hex string"
    }
  ]);
  assert.deepEqual(result.missing, []);
  assert.deepEqual(result.mismatches, []);
});
