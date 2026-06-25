#!/usr/bin/env node

import { createHash } from "node:crypto";
import { access, readFile } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_MANIFEST_PATH = "docs/ai/kairox-skills/manifest.json";
const SKILL_PATH_PREFIX = ".agents/skills/";
const SKILL_PATH_SUFFIX = "/SKILL.md";
const SHA256_RE = /^[a-f0-9]{64}$/;
const UPDATED_AT_RE = /^\d{4}-\d{2}-\d{2}$/;

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

function posixPath(path) {
  return path.split(sep).join("/");
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

async function fileExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

function defaultSuggestedRefreshText() {
  return [
    `Refresh ${DEFAULT_MANIFEST_PATH} by recomputing sha256 values from local .agents/skills/<name>/SKILL.md files.`,
    "Keep the checker read-only; update the tracked manifest in the same change that intentionally changes a local Kairox skill."
  ].join(" ");
}

function localCheckoutForWorktree(repoRoot) {
  const parts = resolve(repoRoot).split(sep);
  const index = parts.lastIndexOf(".worktrees");
  if (index <= 0) {
    return null;
  }
  const root = parts.slice(0, index).join(sep);
  return root === "" ? sep : root;
}

async function resolveSkillPath(repoRoot, manifestPath) {
  const repoCandidate = resolve(repoRoot, manifestPath);
  if (await fileExists(repoCandidate)) {
    return {
      fullPath: repoCandidate,
      localSkillRoot: repoRoot
    };
  }

  const checkoutRoot = localCheckoutForWorktree(repoRoot);
  if (checkoutRoot !== null) {
    const checkoutCandidate = resolve(checkoutRoot, manifestPath);
    if (await fileExists(checkoutCandidate)) {
      return {
        fullPath: checkoutCandidate,
        localSkillRoot: checkoutRoot
      };
    }
  }

  return {
    fullPath: repoCandidate,
    localSkillRoot: repoRoot
  };
}

function validateManifest(manifest) {
  const invalid = [];

  if (!isObject(manifest)) {
    return [
      {
        code: "invalid_manifest",
        field: "manifest",
        message: "manifest must be a JSON object"
      }
    ];
  }

  if (manifest.version !== 1) {
    invalid.push({
      code: "invalid_manifest",
      field: "version",
      message: "version must be 1"
    });
  }

  if (!Array.isArray(manifest.skills)) {
    invalid.push({
      code: "invalid_manifest",
      field: "skills",
      message: "skills must be an array"
    });
    return invalid;
  }

  const names = new Set();
  for (const [index, entry] of manifest.skills.entries()) {
    if (!isObject(entry)) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "entry",
        message: `skills[${index}] must be an object`
      });
      continue;
    }

    const missingField = ["name", "path", "sha256", "updated_at"].find(
      (field) => entry[field] === undefined
    );
    if (missingField !== undefined) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: missingField,
        message:
          missingField === "sha256"
            ? `skills[${index}].sha256 must be a sha256 hex string`
            : `skills[${index}].${missingField} is required`
      });
      continue;
    }

    if (typeof entry.name !== "string" || entry.name.trim() === "") {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "name",
        message: `skills[${index}].name must be a non-empty string`
      });
      continue;
    }

    if (names.has(entry.name)) {
      invalid.push({
        code: "duplicate_manifest_skill",
        index,
        field: "name",
        message: `skills[${index}].name duplicates ${entry.name}`
      });
      continue;
    }
    names.add(entry.name);

    if (typeof entry.path !== "string" || entry.path.trim() === "" || isAbsolute(entry.path)) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "path",
        message: `skills[${index}].path must be a relative path`
      });
      continue;
    }

    const normalizedPath = posixPath(entry.path);
    const expectedPath = `${SKILL_PATH_PREFIX}${entry.name}${SKILL_PATH_SUFFIX}`;
    if (normalizedPath !== expectedPath) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "path",
        message: `skills[${index}].path must be ${expectedPath}`
      });
      continue;
    }

    if (typeof entry.sha256 !== "string" || !SHA256_RE.test(entry.sha256)) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "sha256",
        message: `skills[${index}].sha256 must be a sha256 hex string`
      });
      continue;
    }

    if (typeof entry.updated_at !== "string" || !UPDATED_AT_RE.test(entry.updated_at)) {
      invalid.push({
        code: "invalid_manifest_entry",
        index,
        field: "updated_at",
        message: `skills[${index}].updated_at must use YYYY-MM-DD`
      });
    }
  }

  return invalid;
}

export async function readSkillManifest(manifestPath) {
  const raw = await readFile(manifestPath, "utf8");
  return JSON.parse(raw);
}

export async function evaluateSkillSync({
  repoRoot = process.cwd(),
  manifestPath = join(repoRoot, DEFAULT_MANIFEST_PATH)
} = {}) {
  const resolvedRepoRoot = resolve(repoRoot);
  const resolvedManifestPath = resolve(manifestPath);
  const result = {
    ok: false,
    repoRoot: resolvedRepoRoot,
    manifestPath: resolvedManifestPath,
    localSkillRoot: resolvedRepoRoot,
    suggestedRefreshText: defaultSuggestedRefreshText(),
    invalid: [],
    missing: [],
    mismatches: []
  };

  let manifest;
  try {
    manifest = await readSkillManifest(resolvedManifestPath);
  } catch (error) {
    result.invalid.push({
      code: "invalid_manifest",
      field: "manifest",
      message: `unable to read manifest: ${error.message}`
    });
    return result;
  }

  result.invalid.push(...validateManifest(manifest));
  if (result.invalid.length > 0) {
    return result;
  }

  for (const entry of manifest.skills) {
    const resolved = await resolveSkillPath(resolvedRepoRoot, entry.path);
    result.localSkillRoot = resolved.localSkillRoot;

    if (!(await fileExists(resolved.fullPath))) {
      result.missing.push({
        code: "missing_local_skill",
        name: entry.name,
        path: entry.path
      });
      continue;
    }

    const content = await readFile(resolved.fullPath, "utf8");
    const actual = sha256(content);
    if (actual !== entry.sha256) {
      result.mismatches.push({
        code: "hash_mismatch",
        name: entry.name,
        path: entry.path,
        expected: entry.sha256,
        actual
      });
    }
  }

  result.ok =
    result.invalid.length === 0 && result.missing.length === 0 && result.mismatches.length === 0;
  return result;
}

function parseArgs(argv) {
  const options = {
    json: false,
    manifestPath: null
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
      continue;
    }

    if (arg === "--manifest") {
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error("--manifest requires a path");
      }
      options.manifestPath = value;
      index += 1;
      continue;
    }

    throw new Error(`unknown option: ${arg}`);
  }

  return options;
}

function printTextResult(result) {
  if (result.ok) {
    console.log("Kairox skill manifest is synchronized.");
    return;
  }

  console.log("Kairox skill manifest is out of sync.");

  if (result.invalid.length > 0) {
    console.log("\nInvalid manifest entries:");
    for (const item of result.invalid) {
      console.log(`- [${item.code}] ${item.message}`);
    }
  }

  if (result.missing.length > 0) {
    console.log("\nMissing local skills:");
    for (const item of result.missing) {
      console.log(`- [${item.code}] ${item.name}: ${item.path}`);
    }
  }

  if (result.mismatches.length > 0) {
    console.log("\nHash mismatches:");
    for (const item of result.mismatches) {
      console.log(
        `- [${item.code}] ${item.name}: expected ${item.expected}, actual ${item.actual}`
      );
    }
  }

  console.log(`\nSuggested refresh: ${result.suggestedRefreshText}`);
  if (result.localSkillRoot !== result.repoRoot) {
    console.log(`Local skill root: ${relative(result.repoRoot, result.localSkillRoot)}`);
  }
}

async function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    console.error("Usage: node scripts/check-kairox-skill-sync.mjs [--json] [--manifest <path>]");
    process.exitCode = 2;
    return;
  }

  const manifestPath =
    options.manifestPath === null ? undefined : resolve(process.cwd(), options.manifestPath);
  const result = await evaluateSkillSync({
    repoRoot: process.cwd(),
    manifestPath
  });

  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printTextResult(result);
  }

  if (!result.ok) {
    process.exitCode = 1;
  }
}

const isCli =
  process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  await main();
}
