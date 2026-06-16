//! Command risk classification.
//!
//! Commands are bucketed into [`CommandRisk`] tiers based on a base-program
//! allow-list, with subcommand-specific upgrades for tools like `git`,
//! `bun`, `docker`, and `kubectl`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    ReadOnly,
    Write,
    Destructive,
    Unknown,
}

pub const READONLY_COMMANDS: [&str; 53] = [
    "ls", "cat", "head", "tail", "grep", "egrep", "rg", "find", "wc", "sort", "uniq", "diff",
    "echo", "pwd", "which", "whoami", "env", "printenv", "stat", "file", "du", "df", "free",
    "uptime", "ps", "curl", "wget", "git", "gh", "cargo", "rustc", "node", "python3", "python",
    "java", "go", "make", "cmake", "bun", "npm", "npx", "pnpm", "yarn", "pip", "pip3", "test",
    "true", "false", "date", "uname", "hostname", "id", "arch",
];

pub const WRITE_COMMANDS: [&str; 11] = [
    "cp", "mv", "mkdir", "touch", "chmod", "chown", "ln", "tee", "docker", "kubectl", "helm",
];

pub const DESTRUCTIVE_COMMANDS: [&str; 6] = ["rm", "sudo", "su", "mkfs", "dd", "format"];

pub const GIT_WRITE_SUBCOMMANDS: [&str; 10] = [
    "push",
    "commit",
    "merge",
    "rebase",
    "reset",
    "checkout",
    "branch",
    "tag",
    "stash",
    "cherry-pick",
];

pub const BUN_WRITE_SUBCOMMANDS: [&str; 6] =
    ["add", "install", "remove", "update", "publish", "pm"];
pub const NPM_WRITE_SUBCOMMANDS: [&str; 4] = ["install", "uninstall", "publish", "update"];
pub const PIP_WRITE_SUBCOMMANDS: [&str; 2] = ["install", "uninstall"];
pub const CARGO_WRITE_SUBCOMMANDS: [&str; 1] = ["publish"];

pub const DOCKER_WRITE_SUBCOMMANDS: [&str; 8] = [
    "rm", "rmi", "stop", "kill", "build", "run", "push", "compose",
];

pub const KUBECTL_WRITE_SUBCOMMANDS: [&str; 5] = ["delete", "apply", "create", "edit", "patch"];
pub const HELM_WRITE_SUBCOMMANDS: [&str; 4] = ["install", "upgrade", "delete", "rollback"];

pub const GIT_DESTRUCTIVE_SUBCOMMANDS: [&str; 1] = ["clean"];
pub const DOCKER_DESTRUCTIVE_SUBCOMMANDS: [&str; 2] = ["system", "volume"];

fn is_write_subcommand(program: &str, sub: &str) -> bool {
    match program {
        "git" => GIT_WRITE_SUBCOMMANDS.contains(&sub),
        "bun" => BUN_WRITE_SUBCOMMANDS.contains(&sub),
        "npm" => NPM_WRITE_SUBCOMMANDS.contains(&sub),
        "pip" | "pip3" => PIP_WRITE_SUBCOMMANDS.contains(&sub),
        "cargo" => CARGO_WRITE_SUBCOMMANDS.contains(&sub),
        "docker" => DOCKER_WRITE_SUBCOMMANDS.contains(&sub),
        "kubectl" => KUBECTL_WRITE_SUBCOMMANDS.contains(&sub),
        "helm" => HELM_WRITE_SUBCOMMANDS.contains(&sub),
        _ => false,
    }
}

fn is_destructive_subcommand(program: &str, sub: &str, _args: &[&str]) -> bool {
    match program {
        "git" => GIT_DESTRUCTIVE_SUBCOMMANDS.contains(&sub),
        "docker" => DOCKER_DESTRUCTIVE_SUBCOMMANDS.contains(&sub),
        _ => false,
    }
}

pub fn classify_command(program: &str, args: &[&str]) -> CommandRisk {
    let prog = program.trim();

    // Check subcommand upgrades first (most specific)
    if let Some(sub) = args.first().map(|s| s as &str) {
        if is_destructive_subcommand(prog, sub, &args[1..]) {
            return CommandRisk::Destructive;
        }
        if is_write_subcommand(prog, sub) {
            return CommandRisk::Write;
        }
    }

    // Then check base program classification
    if READONLY_COMMANDS.contains(&prog) {
        return CommandRisk::ReadOnly;
    }
    if WRITE_COMMANDS.contains(&prog) {
        return CommandRisk::Write;
    }
    if DESTRUCTIVE_COMMANDS.contains(&prog) {
        return CommandRisk::Destructive;
    }

    CommandRisk::Unknown
}
