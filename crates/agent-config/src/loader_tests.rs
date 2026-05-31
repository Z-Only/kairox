use super::*;

#[test]
fn parses_context_policy_with_defaults_and_overrides() {
    // Defaults: omitting [context] yields the default ContextPolicy.
    let cfg_default: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
"#,
        "test.toml",
    )
    .unwrap();
    assert!(
        (cfg_default.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
        "default threshold should be 0.85, got {}",
        cfg_default.context.auto_compact_threshold
    );
    assert!(cfg_default.context.compactor_profile.is_none());
    assert!(cfg_default.context.max_tool_definition_tokens.is_none());

    // Overrides: explicit values take precedence.
    let cfg_user: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o"
base_url = "https://api.openai.com/v1"

[context]
auto_compact_threshold = 0.7
compactor_profile = "fast"
max_tool_definition_tokens = 25000
"#,
        "test.toml",
    )
    .unwrap();
    assert!((cfg_user.context.auto_compact_threshold - 0.7).abs() < 1e-6);
    assert_eq!(cfg_user.context.compactor_profile.as_deref(), Some("fast"));
    assert_eq!(cfg_user.context.max_tool_definition_tokens, Some(25_000));
}

#[test]
fn parse_error_on_invalid_toml() {
    let toml = "this is not valid toml {{{{";
    let result = load_from_str(toml, "bad.toml");
    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::Parse { path, .. } => assert_eq!(path, "bad.toml"),
        _ => panic!("expected Parse error"),
    }
}

#[test]
fn config_parse_includes_context_policy() {
    // Empty [context] section uses defaults.
    let cfg_empty: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[context]
"#,
        "test.toml",
    )
    .unwrap();
    assert!(
        (cfg_empty.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
        "default should be 0.85"
    );
    assert!(cfg_empty.context.compactor_profile.is_none());
    assert!(cfg_empty.context.max_tool_definition_tokens.is_none());

    // Override works.
    let cfg_override: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[context]
auto_compact_threshold = 0.9
compactor_profile = "fake"
max_tool_definition_tokens = 50000
"#,
        "test.toml",
    )
    .unwrap();
    assert!((cfg_override.context.auto_compact_threshold - 0.9).abs() < 1e-6);
    assert_eq!(
        cfg_override.context.compactor_profile.as_deref(),
        Some("fake")
    );
    assert_eq!(
        cfg_override.context.max_tool_definition_tokens,
        Some(50_000)
    );
}

#[test]
fn config_parse_includes_hooks() {
    let cfg: crate::Config = crate::loader::load_from_str(
        r#"
[features]
hooks = false

[hooks.Stop.verify]
matcher = "*"
command = "cargo test --workspace --all-targets"
status_message = "Running workspace tests"
timeout_secs = 120
enabled = true

[hooks.PreToolUse.block_rm]
matcher = "shell"
command = "python3 .kairox/hooks/pre_tool.py"
enabled = false
"#,
        "test.toml",
    )
    .unwrap();

    assert!(!cfg.features.hooks);
    assert_eq!(cfg.hooks.len(), 2);
    let verify = cfg
        .hooks
        .iter()
        .find(|hook| hook.event == crate::HookEvent::Stop && hook.id == "verify")
        .expect("Stop.verify hook should parse");
    assert_eq!(verify.matcher.as_deref(), Some("*"));
    assert_eq!(verify.command, "cargo test --workspace --all-targets");
    assert_eq!(
        verify.status_message.as_deref(),
        Some("Running workspace tests")
    );
    assert_eq!(verify.timeout_secs, Some(120));
    assert!(verify.enabled);
    let pre_tool = cfg
        .hooks
        .iter()
        .find(|hook| hook.event == crate::HookEvent::PreToolUse && hook.id == "block_rm")
        .expect("PreToolUse.block_rm hook should parse");
    assert!(!pre_tool.enabled);
}

#[test]
fn config_parse_includes_lsp_and_dap_servers() {
    let cfg = crate::loader::load_from_str(
        r#"
[lsp_servers.rust-analyzer]
command = "rust-analyzer"
args = ["--stdio"]
cwd = "/workspace"
languages = ["rust"]
file_patterns = ["*.rs"]
initialization_options = { check = { command = "clippy" } }
auto_start = false

[lsp_servers.rust-analyzer.env]
RA_LOG = "info"

[dap_servers.lldb]
command = "codelldb"
args = ["--port", "0"]
cwd = "/workspace"
languages = ["rust"]

[dap_servers.lldb.env]
RUST_LOG = "debug"
"#,
        "lsp.toml",
    )
    .expect("LSP/DAP config should parse");

    let (lsp_id, lsp) = cfg
        .lsp_servers
        .iter()
        .find(|(id, _)| id == "rust-analyzer")
        .expect("rust-analyzer LSP server is loaded");
    assert_eq!(lsp_id, "rust-analyzer");
    assert_eq!(lsp.command, "rust-analyzer");
    assert_eq!(lsp.args, vec!["--stdio"]);
    assert_eq!(lsp.cwd.as_deref(), Some("/workspace"));
    assert_eq!(lsp.languages, vec!["rust"]);
    assert_eq!(lsp.file_patterns, vec!["*.rs"]);
    assert_eq!(lsp.env.get("RA_LOG").map(String::as_str), Some("info"));
    assert_eq!(
        lsp.initialization_options
            .as_ref()
            .and_then(|value| value.pointer("/check/command"))
            .and_then(serde_json::Value::as_str),
        Some("clippy")
    );
    assert!(!lsp.auto_start);

    let lsp_defs = cfg.lsp_server_defs();
    let lsp_def = lsp_defs
        .iter()
        .find(|server| server.name == "rust-analyzer")
        .expect("LSP server converts to runtime definition");
    assert_eq!(lsp_def.command, "rust-analyzer");
    assert_eq!(lsp_def.args, vec!["--stdio"]);
    assert_eq!(lsp_def.cwd.as_deref(), Some("/workspace"));
    assert_eq!(lsp_def.languages, vec!["rust"]);
    assert_eq!(lsp_def.file_patterns, vec!["*.rs"]);
    assert_eq!(lsp_def.env.get("RA_LOG").map(String::as_str), Some("info"));
    assert_eq!(
        lsp_def
            .initialization_options
            .as_ref()
            .and_then(|value| value.pointer("/check/command"))
            .and_then(serde_json::Value::as_str),
        Some("clippy")
    );

    let (dap_id, dap) = cfg
        .dap_servers
        .iter()
        .find(|(id, _)| id == "lldb")
        .expect("LLDB DAP server is loaded");
    assert_eq!(dap_id, "lldb");
    assert_eq!(dap.command, "codelldb");
    assert_eq!(dap.args, vec!["--port", "0"]);
    assert_eq!(dap.cwd.as_deref(), Some("/workspace"));
    assert_eq!(dap.languages, vec!["rust"]);
    assert_eq!(dap.env.get("RUST_LOG").map(String::as_str), Some("debug"));

    let dap_defs = cfg.dap_server_defs();
    let dap_def = dap_defs
        .iter()
        .find(|server| server.name == "lldb")
        .expect("DAP server converts to runtime definition");
    assert_eq!(dap_def.command, "codelldb");
    assert_eq!(dap_def.args, vec!["--port", "0"]);
    assert_eq!(dap_def.cwd.as_deref(), Some("/workspace"));
    assert_eq!(dap_def.languages, vec!["rust"]);
    assert_eq!(
        dap_def.env.get("RUST_LOG").map(String::as_str),
        Some("debug")
    );
}

#[test]
fn lsp_and_dap_parse_errors_include_server_id() {
    let lsp_error = crate::loader::load_from_str(
        r#"
[lsp_servers.bad-lsp]
args = ["--stdio"]
"#,
        "bad-lsp.toml",
    )
    .expect_err("missing LSP command should fail");
    match lsp_error {
        ConfigError::Parse { path, message } => {
            assert_eq!(path, "bad-lsp.toml");
            assert!(message.contains("lsp_server 'bad-lsp'"));
        }
        ConfigError::Io(error) => panic!("unexpected IO error: {error}"),
    }

    let dap_error = crate::loader::load_from_str(
        r#"
[dap_servers.bad-dap]
args = ["--port", "0"]
"#,
        "bad-dap.toml",
    )
    .expect_err("missing DAP command should fail");
    match dap_error {
        ConfigError::Parse { path, message } => {
            assert_eq!(path, "bad-dap.toml");
            assert!(message.contains("dap_server 'bad-dap'"));
        }
        ConfigError::Io(error) => panic!("unexpected IO error: {error}"),
    }
}
