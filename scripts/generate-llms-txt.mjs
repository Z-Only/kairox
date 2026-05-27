#!/usr/bin/env node
import { readdir, readFile, writeFile, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..");
const siteRoot = join(repoRoot, "site");
const distRoot = join(siteRoot, ".vitepress", "dist");

const SITE_URL = "https://z-only.github.io/kairox";

const SKIP_DIRS = new Set([".vitepress", "public", "zh"]);

async function walk(dir) {
  const out = [];
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith(".")) continue;
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      out.push(...(await walk(fullPath)));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      out.push(fullPath);
    }
  }
  return out;
}

function frontmatter(raw) {
  if (!raw.startsWith("---")) return { fm: {}, body: raw };
  const end = raw.indexOf("\n---", 3);
  if (end < 0) return { fm: {}, body: raw };
  const block = raw.slice(3, end).trim();
  const body = raw.slice(end + 4).replace(/^\n/, "");
  const fm = {};
  for (const line of block.split("\n")) {
    const m = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (m) {
      let value = m[2].trim();
      value = value.replace(/^"(.*)"$/, "$1").replace(/^'(.*)'$/, "$1");
      fm[m[1]] = value;
    }
  }
  return { fm, body };
}

function firstHeading(body) {
  const lines = body.split("\n");
  for (const line of lines) {
    const m = line.match(/^#\s+(.+?)\s*$/);
    if (m) return m[1].trim();
  }
  return null;
}

function firstParagraph(body) {
  const noHeading = body.replace(/^#+\s+.+$/gm, "").trim();
  for (const block of noHeading.split(/\n\s*\n/)) {
    const cleaned = block.trim();
    if (!cleaned) continue;
    if (cleaned.startsWith("<")) continue;
    if (cleaned.startsWith("```")) continue;
    return cleaned.replace(/\s+/g, " ").slice(0, 220);
  }
  return "";
}

function stripMarkdown(body) {
  return body
    .replace(/```[\s\S]*?```/g, (block) => block)
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<style[\s\S]*?<\/style>/gi, "")
    .replace(/<[^>]+>/g, "")
    .trim();
}

function pathToUrl(absPath) {
  const rel = relative(siteRoot, absPath).split(sep).join("/");
  if (rel === "index.md") return `${SITE_URL}/`;
  if (rel.endsWith("/index.md")) {
    return `${SITE_URL}/${rel.slice(0, -"/index.md".length)}/`;
  }
  return `${SITE_URL}/${rel.replace(/\.md$/, "")}`;
}

async function main() {
  if (!existsSync(distRoot)) {
    console.warn(
      `[llms-txt] dist not found at ${distRoot}; running site build is required before this script. Skipping.`
    );
    return;
  }
  const files = (await walk(siteRoot)).sort();
  const index = [];
  const full = [];
  for (const file of files) {
    const raw = await readFile(file, "utf8");
    const { fm, body } = frontmatter(raw);
    const title = fm.title || firstHeading(body) || relative(siteRoot, file);
    const summary = firstParagraph(body);
    const url = pathToUrl(file);
    index.push(`- [${title}](${url}) — ${summary}`);
    full.push(`# ${title}\n\nSource: ${url}\n\n${stripMarkdown(body)}\n`);
  }
  const header = [
    "# Kairox documentation",
    "",
    "> Local-first AI agent workbench with a shared Rust core, TUI, and Tauri desktop GUI.",
    "",
    "This file lists every English documentation page on the Kairox site so language models can ingest the canonical content. The Chinese mirror lives under /zh and is not duplicated here.",
    "",
    "## Pages",
    ""
  ].join("\n");
  await writeFile(join(distRoot, "llms.txt"), `${header}${index.join("\n")}\n`, "utf8");
  await writeFile(
    join(distRoot, "llms-full.txt"),
    `${header.replace("## Pages", "## Content")}${full.join("\n---\n\n")}\n`,
    "utf8"
  );
  const indexStat = await stat(join(distRoot, "llms.txt"));
  const fullStat = await stat(join(distRoot, "llms-full.txt"));
  console.log(
    `[llms-txt] wrote llms.txt (${indexStat.size} bytes) and llms-full.txt (${fullStat.size} bytes) for ${files.length} pages`
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
