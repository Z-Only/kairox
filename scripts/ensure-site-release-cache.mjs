import { mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const cachePath = join(repoRoot, "site/.vitepress/cache/release.json");

if (!existsSync(cachePath)) {
  await mkdir(dirname(cachePath), { recursive: true });
  await writeFile(cachePath, "{}\n");
}
