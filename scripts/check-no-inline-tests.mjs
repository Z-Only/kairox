import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const cratesDir = join(repoRoot, "crates");

// Kairox convention: Rust unit tests live in `#[path] *_tests.rs` sibling files,
// declared in the source file as `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`
// (a module declaration, NO brace body). An inline `#[cfg(test)] mod tests { ... }`
// block is a regression and must be extracted. This script guards against that.

/** Recursively collect `*.rs` files under a directory. */
function collectRustFiles(dir) {
  const out = [];
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...collectRustFiles(full));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Blank out comments and string/char-like literals while preserving byte length
 * and newline positions, so regex offsets still map to correct line numbers.
 * This avoids false positives from `mod tests {}` appearing inside comments or
 * string literals (e.g. doc examples or historical-context comments).
 */
function blankNonCode(src) {
  const n = src.length;
  const out = Array.from({ length: n });
  let i = 0;
  while (i < n) {
    const c = src[i];
    const c2 = src[i + 1];

    // line comment: // ... \n
    if (c === "/" && c2 === "/") {
      while (i < n && src[i] !== "\n") {
        out[i] = " ";
        i++;
      }
      continue;
    }

    // block comment: /* ... */ (may span lines, no nesting handled — Rust allows
    // nesting but unterminated/nested edge cases only risk over-blanking, never a
    // missed real attribute followed by code outside the comment)
    if (c === "/" && c2 === "*") {
      out[i] = " ";
      out[i + 1] = " ";
      i += 2;
      while (i < n && !(src[i] === "*" && src[i + 1] === "/")) {
        out[i] = src[i] === "\n" ? "\n" : " ";
        i++;
      }
      if (i < n) {
        out[i] = " ";
        out[i + 1] = " ";
        i += 2;
      }
      continue;
    }

    // raw string: r"..." / r#"..."# / r##"..."## ...
    if (c === "r" && (c2 === '"' || c2 === "#")) {
      let j = i + 1;
      let hashes = 0;
      while (src[j] === "#") {
        hashes++;
        j++;
      }
      if (src[j] === '"') {
        for (let k = i; k <= j; k++) out[k] = " ";
        j++;
        const close = '"' + "#".repeat(hashes);
        while (j < n && src.substr(j, close.length) !== close) {
          out[j] = src[j] === "\n" ? "\n" : " ";
          j++;
        }
        for (let k = 0; k < close.length && j < n; k++, j++) out[j] = " ";
        i = j;
        continue;
      }
    }

    // normal string: "..." with \" / \\ escapes
    if (c === '"') {
      out[i] = " ";
      i++;
      while (i < n && src[i] !== '"') {
        if (src[i] === "\\") {
          out[i] = " ";
          if (i + 1 < n) out[i + 1] = src[i + 1] === "\n" ? "\n" : " ";
          i += 2;
          continue;
        }
        out[i] = src[i] === "\n" ? "\n" : " ";
        i++;
      }
      if (i < n) {
        out[i] = " ";
        i++;
      }
      continue;
    }

    out[i] = c;
    i++;
  }
  for (let k = 0; k < n; k++) {
    if (out[k] === undefined) out[k] = src[k];
  }
  return out.join("");
}

// A VIOLATION is `#[cfg(test)]` (tolerating intervening attributes like
// `#[path = "..."]` and whitespace, plus optional `pub`/`pub(crate)`) attached to
// a `mod <ident>` with an inline brace body `{`. The allowed declaration form ends
// in `;` and is intentionally NOT matched.
const INLINE_TEST_MOD =
  /#\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s*(?:\([^)]*\)\s*)?)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/g;

function isExcluded(relPath) {
  const segments = relPath.split(sep);
  const base = segments[segments.length - 1];
  // Extracted sibling test files: these ARE the expected home of inline test items.
  if (base.endsWith("_tests.rs")) return true;
  // Grouped test files under a `src/**/tests/` subdirectory.
  const srcIdx = segments.indexOf("src");
  if (srcIdx !== -1 && segments.slice(srcIdx + 1, -1).includes("tests")) {
    return true;
  }
  return false;
}

const violations = [];

let crates;
try {
  crates = readdirSync(cratesDir, { withFileTypes: true });
} catch {
  console.error(`Could not read crates directory at ${cratesDir}`);
  process.exit(1);
}

for (const crate of crates) {
  if (!crate.isDirectory()) continue;
  const srcDir = join(cratesDir, crate.name, "src");
  let srcStat;
  try {
    srcStat = statSync(srcDir);
  } catch {
    continue;
  }
  if (!srcStat.isDirectory()) continue;

  for (const file of collectRustFiles(srcDir)) {
    const relPath = relative(repoRoot, file);
    if (isExcluded(relPath)) continue;

    const blanked = blankNonCode(readFileSync(file, "utf8"));
    INLINE_TEST_MOD.lastIndex = 0;
    let match;
    while ((match = INLINE_TEST_MOD.exec(blanked)) !== null) {
      const line = blanked.slice(0, match.index).split("\n").length;
      violations.push({ relPath, line });
    }
  }
}

if (violations.length > 0) {
  console.error("Found inline #[cfg(test)] test module(s) with a brace body:");
  for (const { relPath, line } of violations) {
    console.error(
      `${relPath}:${line}: inline #[cfg(test)] module — extract to a #[path] *_tests.rs sibling`
    );
  }
  console.error(
    "\nKairox keeps Rust unit tests in #[path] sibling *_tests.rs files. Replace the inline" +
      '\nblock with a declaration, e.g. `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`,' +
      "\nand move the test body into that sibling file."
  );
  process.exit(1);
}

console.log("No inline #[cfg(test)] test modules found in crates/**/src.");
