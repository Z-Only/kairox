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

const READONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "egrep", "rg", "find", "wc", "sort", "uniq", "diff",
    "echo", "pwd", "which", "whoami", "env", "printenv", "stat", "file", "du", "df", "free",
    "uptime", "ps", "curl", "wget", "git", "gh", "cargo", "rustc", "node", "python3", "python",
    "java", "go", "make", "cmake", "bun", "npm", "npx", "pnpm", "yarn", "pip", "pip3", "test",
    "true", "false", "date", "uname", "hostname", "id", "arch",
];

const WRITE_COMMANDS: &[&str] = &[
    "cp", "mv", "mkdir", "touch", "chmod", "chown", "ln", "tee", "docker", "kubectl", "helm",
];

const DESTRUCTIVE_COMMANDS: &[&str] = &["rm", "sudo", "su", "mkfs", "dd", "format"];

fn is_write_subcommand(program: &str, sub: &str) -> bool {
    match program {
        "git" => matches!(
            sub,
            "push"
                | "commit"
                | "merge"
                | "rebase"
                | "reset"
                | "checkout"
                | "branch"
                | "tag"
                | "stash"
                | "cherry-pick"
        ),
        "bun" => matches!(
            sub,
            "add" | "install" | "remove" | "update" | "publish" | "pm"
        ),
        "npm" => matches!(sub, "install" | "uninstall" | "publish" | "update"),
        "pip" | "pip3" => matches!(sub, "install" | "uninstall"),
        "cargo" => matches!(sub, "publish"),
        "docker" => matches!(
            sub,
            "rm" | "rmi" | "stop" | "kill" | "build" | "run" | "push" | "compose"
        ),
        "kubectl" => matches!(sub, "delete" | "apply" | "create" | "edit" | "patch"),
        "helm" => matches!(sub, "install" | "upgrade" | "delete" | "rollback"),
        _ => false,
    }
}

fn is_destructive_subcommand(program: &str, sub: &str, _args: &[&str]) -> bool {
    match program {
        "git" => matches!(sub, "clean"),
        "docker" => matches!(sub, "system" | "volume"),
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
