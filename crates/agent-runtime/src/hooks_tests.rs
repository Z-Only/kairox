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
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig::default(),
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
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig::default(),
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
