import { readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";
import { describe, expect, it } from "vitest";

const srcDir = dirname(dirname(fileURLToPath(import.meta.url)));

function findTestFolders(dir: string, found: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    if (!statSync(fullPath).isDirectory()) continue;
    if (entry === "__tests__") {
      found.push(relative(srcDir, fullPath));
      continue;
    }
    findTestFolders(fullPath, found);
  }
  return found;
}

describe("GUI test layout", () => {
  it("keeps tests co-located with the implementation files", () => {
    expect(findTestFolders(srcDir)).toEqual([]);
  });
});
