const { existsSync, symlinkSync, lstatSync, rmSync } = require("node:fs");
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const isCi = !!process.env.CI || process.env.NODE_ENV === "production";
if (isCi) {
  process.exit(0);
}

const huskyBin =
  process.platform === "win32"
    ? path.join(process.cwd(), "node_modules", ".bin", "husky.cmd")
    : path.join(process.cwd(), "node_modules", ".bin", "husky");

if (!existsSync(huskyBin)) {
  process.exit(0);
}

const result = spawnSync(huskyBin, [], { stdio: "inherit", shell: false });
if (result.status !== 0 && result.status !== null) {
  process.exit(result.status);
}

// Fix git worktree hooks: husky sets core.hooksPath=".husky/_" which git
// resolves relative to GIT_DIR. For worktrees, GIT_DIR is
// <repo>/.git/worktrees/<name>/, so the hooks directory is not found.
// We create a symlink <GIT_DIR>/.husky -> <worktree-root>/.husky
// so that git can locate and execute hooks from worktrees.
const gitDirResult = spawnSync("git", ["rev-parse", "--git-dir"], {
  encoding: "utf-8",
  shell: false
});
const gitDir = gitDirResult.stdout.trim();

if (gitDir && gitDir.includes("/worktrees/")) {
  const hooksDir = path.join(gitDir, ".husky");
  const targetDir = path.join(process.cwd(), ".husky");

  if (existsSync(targetDir) && existsSync(path.join(targetDir, "_"))) {
    if (lstatSync(hooksDir, { throwIfNoEntry: false })) {
      rmSync(hooksDir, { force: true, recursive: true });
    }

    const relativePath = path.relative(path.dirname(hooksDir), targetDir);
    try {
      symlinkSync(relativePath, hooksDir);
      console.log(
        `husky - linked worktree hooks: ${hooksDir} -> ${relativePath}`
      );
    } catch (err) {
      console.warn(`husky - could not symlink worktree hooks: ${err.message}`);
      console.warn("husky - hooks may not work in this worktree");
    }
  }
}
