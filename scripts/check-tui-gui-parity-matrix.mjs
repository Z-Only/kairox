import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const docPath = join(repoRoot, "docs/testing/tui-gui-parity-matrix.md");
const doc = readFileSync(docPath, "utf8");

function listFiles(dir, predicate) {
  return readdirSync(join(repoRoot, dir), { withFileTypes: true })
    .filter((entry) => entry.isFile() && predicate(entry.name))
    .map((entry) => join(dir, entry.name))
    .sort();
}

function generatedCommandNames() {
  const generatedPath = join(repoRoot, "apps/agent-gui/src/generated/commands.ts");
  const generated = readFileSync(generatedPath, "utf8");
  const body = generated.match(/export const commands = \{([\s\S]*?)\n\};\n\n\/\* Types \*\//)?.[1];
  if (!body) {
    throw new Error(`Could not locate generated commands object in ${generatedPath}`);
  }
  return [...body.matchAll(/^  ([A-Za-z][A-Za-z0-9]+):/gm)].map((match) => match[1]).sort();
}

const requiredRefs = [
  ...generatedCommandNames().map((name) => ({
    kind: "generated command",
    ref: `\`${name}\``
  })),
  ...listFiles(
    "apps/agent-gui/src/components",
    (name) => name.endsWith("SettingsPane.vue") || name === "CatalogSourcesSettings.vue"
  ).map((path) => ({ kind: "settings pane", ref: path })),
  ...listFiles("apps/agent-gui/src/components/skills", (name) => name.endsWith("Settings.vue")).map(
    (path) => ({ kind: "settings pane", ref: path })
  ),
  ...listFiles("apps/agent-gui/src/views/settings", (name) => name.endsWith("Settings.vue")).map(
    (path) => ({ kind: "settings pane", ref: path })
  ),
  ...listFiles("apps/agent-gui/e2e", (name) => name.endsWith(".spec.ts")).map((path) => ({
    kind: "GUI e2e spec",
    ref: path
  })),
  ...listFiles("apps/agent-gui/e2e-pilot", (name) => name.endsWith(".toml")).map((path) => ({
    kind: "pilot scenario",
    ref: path
  }))
];

const missing = requiredRefs.filter(({ ref }) => !doc.includes(ref));

if (missing.length > 0) {
  console.error("TUI/GUI parity matrix is missing required references:");
  for (const item of missing) {
    console.error(`- ${item.kind}: ${item.ref}`);
  }
  console.error(`\nUpdate ${relative(repoRoot, docPath)} before merging parity-affecting changes.`);
  process.exit(1);
}

console.log(`TUI/GUI parity matrix references ${requiredRefs.length} current surfaces.`);
