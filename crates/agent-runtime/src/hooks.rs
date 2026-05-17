use std::path::PathBuf;
use std::time::Duration;

use futures::future::join_all;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone)]
pub struct HookRunContext {
    pub event: agent_config::HookEvent,
    pub matcher_value: String,
    pub cwd: PathBuf,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunReport {
    pub id: String,
    pub event: agent_config::HookEvent,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[derive(Debug, Serialize)]
struct HookCommandPayload<'a> {
    event: &'a str,
    matcher: &'a str,
    payload: &'a serde_json::Value,
}

pub async fn run_hooks(
    config: &agent_config::Config,
    context: HookRunContext,
) -> Vec<HookRunReport> {
    if !config.features.hooks {
        return Vec::new();
    }

    let matching_hooks: Vec<agent_config::HookConfig> = config
        .hooks
        .iter()
        .filter(|hook| {
            hook.enabled
                && hook.event == context.event
                && hook_matches(hook, &context.matcher_value)
        })
        .cloned()
        .collect();

    join_all(
        matching_hooks
            .into_iter()
            .map(|hook| run_command_hook(hook, context.clone())),
    )
    .await
}

pub async fn run_hooks_logged(
    config: &agent_config::Config,
    event: agent_config::HookEvent,
    matcher_value: &str,
    root_path: Option<&std::path::Path>,
    payload: serde_json::Value,
) {
    let cwd = root_path
        .map(std::path::Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let reports = run_hooks(
        config,
        HookRunContext {
            event,
            matcher_value: matcher_value.to_string(),
            cwd,
            payload,
        },
    )
    .await;
    for report in reports {
        if !report.success {
            tracing::warn!(
                hook_id = %report.id,
                event = %report.event,
                exit_code = ?report.exit_code,
                timed_out = report.timed_out,
                stderr = %report.stderr,
                "hook command failed"
            );
        }
    }
}

pub fn hook_matches(hook: &agent_config::HookConfig, matcher_value: &str) -> bool {
    let Some(matcher) = hook
        .matcher
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return true;
    };
    if matcher == "*" {
        return true;
    }
    matcher
        .split('|')
        .map(str::trim)
        .any(|candidate| candidate == matcher_value)
}

async fn run_command_hook(
    hook: agent_config::HookConfig,
    context: HookRunContext,
) -> HookRunReport {
    let mut command = shell_command(&hook.command);
    command
        .current_dir(&context.cwd)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return HookRunReport {
                id: hook.id,
                event: hook.event,
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: error.to_string(),
                timed_out: false,
            }
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let payload = HookCommandPayload {
            event: context.event.as_str(),
            matcher: &context.matcher_value,
            payload: &context.payload,
        };
        match serde_json::to_vec(&payload) {
            Ok(bytes) => {
                let _ = stdin.write_all(&bytes).await;
            }
            Err(error) => {
                let _ = stdin.write_all(error.to_string().as_bytes()).await;
            }
        }
    }

    let timeout = Duration::from_secs(hook.timeout_secs.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS));
    let output = tokio::time::timeout(timeout, child.wait_with_output()).await;
    match output {
        Ok(Ok(output)) => HookRunReport {
            id: hook.id,
            event: hook.event,
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            timed_out: false,
        },
        Ok(Err(error)) => HookRunReport {
            id: hook.id,
            event: hook.event,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: error.to_string(),
            timed_out: false,
        },
        Err(_) => HookRunReport {
            id: hook.id,
            event: hook.event,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("hook timed out after {}s", timeout.as_secs()),
            timed_out: true,
        },
    }
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(
        id: &str,
        event: agent_config::HookEvent,
        matcher: Option<&str>,
    ) -> agent_config::HookConfig {
        agent_config::HookConfig {
            id: id.into(),
            event,
            matcher: matcher.map(str::to_string),
            command: "true".into(),
            status_message: None,
            timeout_secs: None,
            enabled: true,
        }
    }

    #[test]
    fn hook_matches_wildcard_exact_and_pipe_patterns() {
        assert!(hook_matches(
            &hook("all", agent_config::HookEvent::PreToolUse, None),
            "shell"
        ));
        assert!(hook_matches(
            &hook("all", agent_config::HookEvent::PreToolUse, Some("*")),
            "shell"
        ));
        assert!(hook_matches(
            &hook("shell", agent_config::HookEvent::PreToolUse, Some("shell")),
            "shell"
        ));
        assert!(hook_matches(
            &hook(
                "tools",
                agent_config::HookEvent::PreToolUse,
                Some("fs.read|shell")
            ),
            "shell"
        ));
        assert!(!hook_matches(
            &hook(
                "tools",
                agent_config::HookEvent::PreToolUse,
                Some("fs.read|fs.write")
            ),
            "shell"
        ));
    }

    #[tokio::test]
    async fn run_hooks_sends_event_payload_to_command_stdin() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let config = agent_config::Config {
            profiles: vec![],
            mcp_servers: vec![],
            source: agent_config::ConfigSource::Defaults,
            context: agent_config::ContextPolicy::default(),
            disabled_mcp_servers: vec![],
            instructions: None,
            features: agent_config::FeatureFlags { hooks: true },
            hooks: vec![agent_config::HookConfig {
                id: "capture".into(),
                event: agent_config::HookEvent::Stop,
                matcher: Some("*".into()),
                command: "python3 -c 'import json,pathlib,sys; data=json.load(sys.stdin); pathlib.Path(\"hook.log\").write_text(data[\"event\"] + \":\" + data[\"payload\"][\"reason\"])'".into(),
                status_message: None,
                timeout_secs: Some(5),
                enabled: true,
            }],
        };
        let context = HookRunContext {
            event: agent_config::HookEvent::Stop,
            matcher_value: "complete".into(),
            cwd: dir.path().to_path_buf(),
            payload: serde_json::json!({ "reason": "complete" }),
        };

        let reports = run_hooks(&config, context).await;

        assert_eq!(reports.len(), 1);
        assert!(reports[0].success);
        let log = std::fs::read_to_string(dir.path().join("hook.log")).expect("hook log");
        assert_eq!(log, "Stop:complete");
    }

    #[tokio::test]
    async fn run_hooks_skips_when_feature_flag_disabled() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let config = agent_config::Config {
            profiles: vec![],
            mcp_servers: vec![],
            source: agent_config::ConfigSource::Defaults,
            context: agent_config::ContextPolicy::default(),
            disabled_mcp_servers: vec![],
            instructions: None,
            features: agent_config::FeatureFlags { hooks: false },
            hooks: vec![agent_config::HookConfig {
                id: "capture".into(),
                event: agent_config::HookEvent::Stop,
                matcher: Some("*".into()),
                command: "touch hook.log".into(),
                status_message: None,
                timeout_secs: Some(5),
                enabled: true,
            }],
        };
        let context = HookRunContext {
            event: agent_config::HookEvent::Stop,
            matcher_value: "complete".into(),
            cwd: dir.path().to_path_buf(),
            payload: serde_json::json!({}),
        };

        let reports = run_hooks(&config, context).await;

        assert!(reports.is_empty());
        assert!(!dir.path().join("hook.log").exists());
    }
}
