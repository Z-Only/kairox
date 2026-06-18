import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const releaseDataPath = "docs/current-release.json";

const DOC_TARGETS = [
  "README.md",
  "ROADMAP.md",
  "docs/ROADMAP.md",
  "site/community/roadmap.md",
  "site/zh/community/roadmap.md",
  "site/concepts/extensibility.md",
  "site/zh/concepts/extensibility.md"
];

const EN_MEMORY_ANCHOR =
  "- Tiktoken-based context budgeting with auto-compaction at a configurable threshold.";
const ZH_MEMORY_ANCHOR = "- 基于 tiktoken 的上下文 budget 控制，达到可配置阈值时自动 compaction。";

const EN_MEMORY_BLOCK = `<!-- current-release:memory-context:start -->
- Workspace RAG with \`WorkspaceRagIndex\`, pluggable embedding backends, and turn-time context injection.
- Profile-scoped external knowledge bases with SQLite FTS today and config models for Tantivy, Bedrock Knowledge Bases, Pinecone, and Weaviate.
<!-- current-release:memory-context:end -->`;

const ZH_MEMORY_BLOCK = `<!-- current-release:memory-context:start -->
- Workspace RAG，包含 \`WorkspaceRagIndex\`、可插拔 embedding backend，以及每 turn 的 context 注入。
- 按 profile 作用域启用的外部知识库：当前支持 SQLite FTS runtime connector，并在配置模型中覆盖 Tantivy、Bedrock Knowledge Bases、Pinecone 与 Weaviate。
<!-- current-release:memory-context:end -->`;

export async function readWorkspaceVersion(root = repoRoot) {
  const cargoToml = await readFile(join(root, "Cargo.toml"), "utf8");
  let inWorkspacePackage = false;
  for (const line of cargoToml.split(/\r?\n/)) {
    if (/^\s*\[/.test(line)) {
      inWorkspacePackage = /^\s*\[workspace\.package\]\s*$/.test(line);
      continue;
    }
    if (!inWorkspacePackage) {
      continue;
    }
    const version = line.match(/^\s*version\s*=\s*"(?<version>[^"]+)"/);
    if (version?.groups?.version) {
      return version.groups.version;
    }
  }
  throw new Error("Could not find workspace.package.version in Cargo.toml");
}

export function deriveReleaseFields(version, releaseDate) {
  const match = version.match(/^(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)(?:[-+].*)?$/);
  if (!match?.groups) {
    throw new Error(`Expected a semver-like version, got ${version}`);
  }

  const major = Number(match.groups.major);
  const minor = Number(match.groups.minor);

  return {
    version,
    releaseDate,
    displayVersion: `v${version}`,
    minorLine: `v${major}.${minor}.x`,
    compatRange: `>=${version} <${major}.${minor + 1}.0`
  };
}

async function readCurrentRelease(root) {
  const fullPath = join(root, releaseDataPath);
  if (!existsSync(fullPath)) {
    return {};
  }
  const parsed = JSON.parse(await readFile(fullPath, "utf8"));
  return parsed;
}

function releaseDateFor(version, releaseData, today) {
  if (releaseData.version === version && releaseData.releaseDate) {
    return releaseData.releaseDate;
  }
  return today;
}

function currentReleaseJson(fields) {
  return `${JSON.stringify(fields, null, 2)}\n`;
}

function upsertCurrentVersionBlock(text, title, fields) {
  const line = `> Current version: **${fields.version}** (${fields.releaseDate}).`;
  const currentVersion = /^> Current version: \*\*[^*]+\*\* \([^)]+\)\.$/m;
  if (currentVersion.test(text)) {
    return text.replace(currentVersion, line);
  }
  return text.replace(new RegExp(`^# ${title}\\n`), `# ${title}\n\n${line}\n`);
}

function replaceMemoryBlock(text, anchor, block) {
  const blockPattern =
    /<!-- current-release:memory-context:start -->[\s\S]*?<!-- current-release:memory-context:end -->/;
  if (blockPattern.test(text)) {
    return text.replace(blockPattern, block);
  }
  if (!text.includes(anchor)) {
    return text;
  }
  return text.replace(anchor, `${anchor}\n${block}`);
}

function syncText(path, text, fields) {
  switch (path) {
    case "README.md":
      return text.replace(
        /current release `v[^`]+`/g,
        `current release \`${fields.displayVersion}\``
      );
    case "ROADMAP.md":
      return upsertCurrentVersionBlock(text, "Roadmap", fields);
    case "docs/ROADMAP.md":
      return text
        .replace(
          /^> Current version: \*\*[^*]+\*\* \([^)]+\)\.$/m,
          `> Current version: **${fields.version}** (${fields.releaseDate}).`
        )
        .replace(
          /^\| Memory \+ context assembly\s+\|.*$/m,
          "| Memory + context assembly                 | ✅ Multi-scope memory + tiktoken budgets + compaction + RAG/KB retrieval | Competitive; local-first RAG and scoped KB connectors are in place   |"
        )
        .replace(
          /^## Phase 4 — Knowledge and retrieval \(v[^)]+\).*$/m,
          `## Phase 4 — Knowledge and retrieval (v${fields.version.split(".").slice(0, 2).join(".")}+) ✅`
        );
    case "site/community/roadmap.md":
      return replaceMemoryBlock(
        text.replace(
          /^## What ships today \(v[^)]+\)$/m,
          `## What ships today (${fields.minorLine})`
        ),
        EN_MEMORY_ANCHOR,
        EN_MEMORY_BLOCK
      );
    case "site/zh/community/roadmap.md":
      return replaceMemoryBlock(
        text.replace(/^## 当前已发布（v[^）]+）$/m, `## 当前已发布（${fields.minorLine}）`),
        ZH_MEMORY_ANCHOR,
        ZH_MEMORY_BLOCK
      );
    case "site/concepts/extensibility.md":
    case "site/zh/concepts/extensibility.md":
      return text.replace(/>=\d+\.\d+\.\d+ <\d+\.\d+\.\d+/g, fields.compatRange);
    default:
      return text;
  }
}

export async function syncReleaseDocs(
  root = repoRoot,
  { write = false, today = new Date().toISOString().slice(0, 10) } = {}
) {
  const version = await readWorkspaceVersion(root);
  const releaseData = await readCurrentRelease(root);
  const fields = deriveReleaseFields(version, releaseDateFor(version, releaseData, today));
  const changedPaths = [];

  const expectedReleaseData = currentReleaseJson(fields);
  const currentReleaseDataPath = join(root, releaseDataPath);
  const currentReleaseData = existsSync(currentReleaseDataPath)
    ? await readFile(currentReleaseDataPath, "utf8")
    : "";
  if (currentReleaseData !== expectedReleaseData) {
    changedPaths.push(releaseDataPath);
    if (write) {
      await mkdir(dirname(currentReleaseDataPath), { recursive: true });
      await writeFile(currentReleaseDataPath, expectedReleaseData);
    }
  }

  for (const path of DOC_TARGETS) {
    const fullPath = join(root, path);
    if (!existsSync(fullPath)) {
      continue;
    }
    const before = await readFile(fullPath, "utf8");
    const after = syncText(path, before, fields);
    if (before !== after) {
      changedPaths.push(path);
      if (write) {
        await writeFile(fullPath, after);
      }
    }
  }

  return { fields, changedPaths };
}

export async function checkReleaseDocs(root = repoRoot, options = {}) {
  const result = await syncReleaseDocs(root, { ...options, write: false });
  return {
    ...result,
    ok: result.changedPaths.length === 0
  };
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const write = args.has("--write");
  const check = args.has("--check") || !write;
  const result = write
    ? await syncReleaseDocs(repoRoot, { write: true })
    : await checkReleaseDocs(repoRoot);

  if (write) {
    if (result.changedPaths.length === 0) {
      console.log("Release docs already match Cargo workspace version.");
    } else {
      console.log(
        `Updated release docs:\n${result.changedPaths.map((path) => `- ${path}`).join("\n")}`
      );
    }
    return;
  }

  if (check && !result.ok) {
    console.error("Release docs are stale. Run `bun run release-docs:sync` and commit the result.");
    for (const path of result.changedPaths) {
      console.error(`- ${relative(repoRoot, join(repoRoot, path))}`);
    }
    process.exit(1);
  }

  console.log("Release docs match Cargo workspace version.");
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
