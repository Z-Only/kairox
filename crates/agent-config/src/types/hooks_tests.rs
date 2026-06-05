use super::*;

// ── HookEvent::as_str / parse round-trip ────────────────────────────

#[test]
fn hook_event_as_str_returns_pascal_case() {
    assert_eq!(HookEvent::SessionStart.as_str(), "SessionStart");
    assert_eq!(HookEvent::UserPromptSubmit.as_str(), "UserPromptSubmit");
    assert_eq!(HookEvent::PreToolUse.as_str(), "PreToolUse");
    assert_eq!(HookEvent::PermissionRequest.as_str(), "PermissionRequest");
    assert_eq!(HookEvent::PostToolUse.as_str(), "PostToolUse");
    assert_eq!(HookEvent::Stop.as_str(), "Stop");
}

#[test]
fn hook_event_parse_roundtrips_all_variants() {
    let variants = [
        HookEvent::SessionStart,
        HookEvent::UserPromptSubmit,
        HookEvent::PreToolUse,
        HookEvent::PermissionRequest,
        HookEvent::PostToolUse,
        HookEvent::Stop,
    ];
    for v in variants {
        let s = v.as_str();
        let parsed = HookEvent::parse(s);
        assert_eq!(parsed, Some(v), "roundtrip failed for {s}");
    }
}

#[test]
fn hook_event_parse_returns_none_for_unknown() {
    assert_eq!(HookEvent::parse(""), None);
    assert_eq!(HookEvent::parse("session_start"), None);
    assert_eq!(HookEvent::parse("STOP"), None);
    assert_eq!(HookEvent::parse("NonExistent"), None);
}

// ── HookEvent Display ───────────────────────────────────────────────

#[test]
fn hook_event_display_matches_as_str() {
    let variants = [
        HookEvent::SessionStart,
        HookEvent::UserPromptSubmit,
        HookEvent::PreToolUse,
        HookEvent::PermissionRequest,
        HookEvent::PostToolUse,
        HookEvent::Stop,
    ];
    for v in variants {
        assert_eq!(format!("{v}"), v.as_str());
    }
}

// ── HookEvent serde ─────────────────────────────────────────────────

#[test]
fn hook_event_serde_roundtrip() {
    let variants = [
        HookEvent::SessionStart,
        HookEvent::UserPromptSubmit,
        HookEvent::PreToolUse,
        HookEvent::PermissionRequest,
        HookEvent::PostToolUse,
        HookEvent::Stop,
    ];
    for v in variants {
        let json = serde_json::to_string(&v).expect("serialize");
        let back: HookEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, v);
    }
}

// ── HookConfig serde ────────────────────────────────────────────────

#[test]
fn hook_config_minimal_json() {
    let json = r#"{
        "id": "fmt",
        "event": "PreToolUse",
        "command": "cargo fmt --check"
    }"#;
    let cfg: HookConfig = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.id, "fmt");
    assert_eq!(cfg.event, HookEvent::PreToolUse);
    assert_eq!(cfg.command, "cargo fmt --check");
    assert!(cfg.matcher.is_none());
    assert!(cfg.status_message.is_none());
    assert!(cfg.timeout_secs.is_none());
    assert!(cfg.enabled, "enabled defaults to true");
}

#[test]
fn hook_config_full_json() {
    let json = r#"{
        "id": "lint",
        "event": "PostToolUse",
        "matcher": "shell\\.exec",
        "command": "eslint .",
        "status_message": "Linting...",
        "timeout_secs": 30,
        "enabled": false
    }"#;
    let cfg: HookConfig = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.id, "lint");
    assert_eq!(cfg.event, HookEvent::PostToolUse);
    assert_eq!(cfg.matcher.as_deref(), Some("shell\\.exec"));
    assert_eq!(cfg.command, "eslint .");
    assert_eq!(cfg.status_message.as_deref(), Some("Linting..."));
    assert_eq!(cfg.timeout_secs, Some(30));
    assert!(!cfg.enabled);
}

#[test]
fn hook_config_equality() {
    let a = HookConfig {
        id: "a".into(),
        event: HookEvent::Stop,
        matcher: None,
        command: "echo done".into(),
        status_message: None,
        timeout_secs: None,
        enabled: true,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

// ── HookConfigToml ──────────────────────────────────────────────────

#[test]
fn hook_config_toml_minimal() {
    let toml_str = r#"
        command = "echo hello"
    "#;
    let cfg: HookConfigToml = toml::from_str(toml_str).expect("parse");
    assert_eq!(cfg.command, "echo hello");
    assert!(cfg.matcher.is_none());
    assert!(cfg.status_message.is_none());
    assert!(cfg.timeout_secs.is_none());
    assert!(cfg.enabled);
}

#[test]
fn hook_config_toml_full() {
    let toml_str = r#"
        matcher = "fs\\.write"
        command = "prettier --write"
        status_message = "Formatting..."
        timeout_secs = 10
        enabled = false
    "#;
    let cfg: HookConfigToml = toml::from_str(toml_str).expect("parse");
    assert_eq!(cfg.matcher.as_deref(), Some("fs\\.write"));
    assert_eq!(cfg.command, "prettier --write");
    assert_eq!(cfg.status_message.as_deref(), Some("Formatting..."));
    assert_eq!(cfg.timeout_secs, Some(10));
    assert!(!cfg.enabled);
}

// ── HookConfigToml::into_hook_config ────────────────────────────────

#[test]
fn into_hook_config_populates_event_and_id() {
    let toml_cfg = HookConfigToml {
        matcher: Some("pattern".into()),
        command: "run-thing".into(),
        status_message: Some("Running...".into()),
        timeout_secs: Some(60),
        enabled: true,
    };
    let hook = toml_cfg.into_hook_config(HookEvent::SessionStart, "my-hook".into());
    assert_eq!(hook.id, "my-hook");
    assert_eq!(hook.event, HookEvent::SessionStart);
    assert_eq!(hook.matcher.as_deref(), Some("pattern"));
    assert_eq!(hook.command, "run-thing");
    assert_eq!(hook.status_message.as_deref(), Some("Running..."));
    assert_eq!(hook.timeout_secs, Some(60));
    assert!(hook.enabled);
}

#[test]
fn into_hook_config_disabled() {
    let toml_cfg = HookConfigToml {
        matcher: None,
        command: "noop".into(),
        status_message: None,
        timeout_secs: None,
        enabled: false,
    };
    let hook = toml_cfg.into_hook_config(HookEvent::Stop, "disabled-hook".into());
    assert_eq!(hook.id, "disabled-hook");
    assert_eq!(hook.event, HookEvent::Stop);
    assert!(!hook.enabled);
}
