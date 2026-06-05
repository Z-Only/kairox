use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_stdio_view() -> McpServerSettingsView {
    McpServerSettingsView {
        id: "test-server".to_string(),
        name: "test-server".to_string(),
        transport: "stdio".to_string(),
        enabled: true,
        runtime_status: "stopped".to_string(),
        trusted: false,
        tool_count: None,
        last_error: None,
        writable: true,
        config_path: None,
        description: Some("A test server".to_string()),
        source: "user".to_string(),
        verified: false,
        diagnostic_summary: String::new(),
    }
}

fn make_sse_view() -> McpServerSettingsView {
    McpServerSettingsView {
        id: "sse-server".to_string(),
        name: "sse-server".to_string(),
        transport: "sse".to_string(),
        enabled: false,
        runtime_status: "stopped".to_string(),
        trusted: false,
        tool_count: None,
        last_error: None,
        writable: true,
        config_path: None,
        description: None,
        source: "user".to_string(),
        verified: false,
        diagnostic_summary: String::new(),
    }
}

fn make_streamable_http_view() -> McpServerSettingsView {
    McpServerSettingsView {
        id: "http-server".to_string(),
        name: "http-server".to_string(),
        transport: "streamable_http".to_string(),
        enabled: true,
        runtime_status: "running".to_string(),
        trusted: false,
        tool_count: None,
        last_error: None,
        writable: true,
        config_path: None,
        description: Some("HTTP transport".to_string()),
        source: "project".to_string(),
        verified: false,
        diagnostic_summary: String::new(),
    }
}

fn make_server_entry() -> ServerEntry {
    ServerEntry {
        id: "test-catalog".to_string(),
        source: "builtin".to_string(),
        display_name: "Test Tool".to_string(),
        summary: "A test tool".to_string(),
        description: "Long description".to_string(),
        categories: vec!["testing".to_string()],
        tags: vec![],
        author: Some("author".to_string()),
        homepage: None,
        version: Some("1.0.0".to_string()),
        trust: "verified".to_string(),
        verified: true,
        icon: None,
        install_spec_json: r#"{"transport":"stdio","command":"node","args":["server.js"],"env":{}}"#
            .to_string(),
        requirements_json: "[]".to_string(),
        default_env_json: r#"[{"key":"API_KEY","label":"API Key","description":"API key","required":true,"secret":true,"default":null}]"#.to_string(),
    }
}

// ---------------------------------------------------------------------------
// ServerDraft — construction
// ---------------------------------------------------------------------------

#[test]
fn server_draft_new_defaults() {
    let draft = ServerDraft::new();

    assert_eq!(draft.name, "");
    assert_eq!(draft.transport, ServerTransportDraft::Stdio);
    assert_eq!(draft.command, "");
    assert_eq!(draft.args_text, "");
    assert_eq!(draft.url, "");
    assert_eq!(draft.description, "");
    assert!(draft.enabled);
}

#[test]
fn server_draft_from_stdio_view() {
    let view = make_stdio_view();
    let draft = ServerDraft::from_view(&view);

    assert_eq!(draft.name, "test-server");
    assert_eq!(draft.transport, ServerTransportDraft::Stdio);
    assert!(draft.enabled);
    assert_eq!(draft.description, "A test server");
}

#[test]
fn server_draft_from_sse_view() {
    let view = make_sse_view();
    let draft = ServerDraft::from_view(&view);

    assert_eq!(draft.transport, ServerTransportDraft::Sse);
    assert!(!draft.enabled);
    assert_eq!(draft.description, "");
}

#[test]
fn server_draft_from_streamable_http_view() {
    let view = make_streamable_http_view();
    let draft = ServerDraft::from_view(&view);

    assert_eq!(draft.transport, ServerTransportDraft::StreamableHttp);
    assert_eq!(draft.description, "HTTP transport");
}

#[test]
fn server_draft_from_unknown_transport_defaults_to_stdio() {
    let mut view = make_stdio_view();
    view.transport = "grpc".to_string();
    let draft = ServerDraft::from_view(&view);

    assert_eq!(draft.transport, ServerTransportDraft::Stdio);
}

// ---------------------------------------------------------------------------
// ServerDraft — to_input
// ---------------------------------------------------------------------------

#[test]
fn server_draft_to_input_returns_none_when_name_empty() {
    let draft = ServerDraft::new();
    assert!(draft.to_input().is_none());
}

#[test]
fn server_draft_to_input_stdio_returns_none_when_command_empty() {
    let mut draft = ServerDraft::new();
    draft.name = "my-server".to_string();
    assert!(draft.to_input().is_none());
}

#[test]
fn server_draft_to_input_stdio_success() {
    let mut draft = ServerDraft::new();
    draft.name = "my-server".to_string();
    draft.command = "node".to_string();
    draft.args_text = "server.js --port 3000".to_string();
    draft.description = "Desc".to_string();

    let input = draft.to_input().expect("should produce input");
    assert_eq!(input.name, "my-server");
    assert!(input.enabled);
    assert_eq!(input.description, Some("Desc".to_string()));
    match &input.transport {
        McpServerSettingsTransport::Stdio { command, args, .. } => {
            assert_eq!(command, "node");
            assert_eq!(args, &["server.js", "--port", "3000"]);
        }
        _ => panic!("expected Stdio transport"),
    }
}

#[test]
fn server_draft_to_input_sse_returns_none_when_url_empty() {
    let mut draft = ServerDraft::new();
    draft.name = "sse".to_string();
    draft.transport = ServerTransportDraft::Sse;
    assert!(draft.to_input().is_none());
}

#[test]
fn server_draft_to_input_sse_success() {
    let mut draft = ServerDraft::new();
    draft.name = "sse".to_string();
    draft.transport = ServerTransportDraft::Sse;
    draft.url = "http://localhost:8080/sse".to_string();

    let input = draft.to_input().unwrap();
    match &input.transport {
        McpServerSettingsTransport::Sse { url, .. } => {
            assert_eq!(url, "http://localhost:8080/sse");
        }
        _ => panic!("expected Sse transport"),
    }
}

#[test]
fn server_draft_to_input_streamable_http_success() {
    let mut draft = ServerDraft::new();
    draft.name = "http".to_string();
    draft.transport = ServerTransportDraft::StreamableHttp;
    draft.url = "http://localhost:9090/mcp".to_string();

    let input = draft.to_input().unwrap();
    match &input.transport {
        McpServerSettingsTransport::StreamableHttp { url, .. } => {
            assert_eq!(url, "http://localhost:9090/mcp");
        }
        _ => panic!("expected StreamableHttp transport"),
    }
}

#[test]
fn server_draft_to_input_trims_whitespace() {
    let mut draft = ServerDraft::new();
    draft.name = "  my-server  ".to_string();
    draft.command = "  node  ".to_string();
    draft.description = "  ".to_string();

    let input = draft.to_input().unwrap();
    assert_eq!(input.name, "my-server");
    assert_eq!(input.description, None);
}

// ---------------------------------------------------------------------------
// ServerDraft — field editing
// ---------------------------------------------------------------------------

#[test]
fn server_draft_push_char_name() {
    let mut draft = ServerDraft::new();
    draft.push_char(ServerEditorField::Name, 'a');
    draft.push_char(ServerEditorField::Name, 'b');
    assert_eq!(draft.name, "ab");
}

#[test]
fn server_draft_push_char_transport_switches() {
    let mut draft = ServerDraft::new();
    assert_eq!(draft.transport, ServerTransportDraft::Stdio);

    draft.push_char(ServerEditorField::Transport, 'e');
    assert_eq!(draft.transport, ServerTransportDraft::Sse);

    draft.push_char(ServerEditorField::Transport, 'H');
    assert_eq!(draft.transport, ServerTransportDraft::StreamableHttp);

    draft.push_char(ServerEditorField::Transport, 'S');
    assert_eq!(draft.transport, ServerTransportDraft::Stdio);

    draft.push_char(ServerEditorField::Transport, 'x');
    assert_eq!(draft.transport, ServerTransportDraft::Stdio);
}

#[test]
fn server_draft_push_char_command_vs_url_by_transport() {
    let mut draft = ServerDraft::new();
    draft.push_char(ServerEditorField::CommandOrUrl, 'n');
    assert_eq!(draft.command, "n");
    assert_eq!(draft.url, "");

    draft.transport = ServerTransportDraft::Sse;
    draft.push_char(ServerEditorField::CommandOrUrl, 'h');
    assert_eq!(draft.command, "n");
    assert_eq!(draft.url, "h");
}

#[test]
fn server_draft_push_char_args_only_in_stdio() {
    let mut draft = ServerDraft::new();
    draft.push_char(ServerEditorField::Args, 'a');
    assert_eq!(draft.args_text, "a");

    draft.transport = ServerTransportDraft::Sse;
    draft.push_char(ServerEditorField::Args, 'b');
    assert_eq!(draft.args_text, "a");
}

#[test]
fn server_draft_push_char_enabled_toggle() {
    let mut draft = ServerDraft::new();
    assert!(draft.enabled);

    draft.push_char(ServerEditorField::Enabled, ' ');
    assert!(!draft.enabled);

    draft.push_char(ServerEditorField::Enabled, 'Y');
    assert!(draft.enabled);

    draft.push_char(ServerEditorField::Enabled, 'N');
    assert!(!draft.enabled);

    draft.push_char(ServerEditorField::Enabled, '1');
    assert!(draft.enabled);

    draft.push_char(ServerEditorField::Enabled, '0');
    assert!(!draft.enabled);
}

#[test]
fn server_draft_backspace() {
    let mut draft = ServerDraft::new();
    draft.name = "abc".to_string();
    draft.backspace(ServerEditorField::Name);
    assert_eq!(draft.name, "ab");

    draft.command = "node".to_string();
    draft.backspace(ServerEditorField::CommandOrUrl);
    assert_eq!(draft.command, "nod");

    draft.transport = ServerTransportDraft::Sse;
    draft.url = "http".to_string();
    draft.backspace(ServerEditorField::CommandOrUrl);
    assert_eq!(draft.url, "htt");

    draft.backspace(ServerEditorField::Transport);
    draft.backspace(ServerEditorField::Enabled);
}

#[test]
fn server_draft_clear_field() {
    let mut draft = ServerDraft::new();
    draft.name = "test".to_string();
    draft.clear_field(ServerEditorField::Name);
    assert_eq!(draft.name, "");

    draft.command = "node".to_string();
    draft.clear_field(ServerEditorField::CommandOrUrl);
    assert_eq!(draft.command, "");

    draft.transport = ServerTransportDraft::Sse;
    draft.url = "http://x".to_string();
    draft.clear_field(ServerEditorField::CommandOrUrl);
    assert_eq!(draft.url, "");
}

// ---------------------------------------------------------------------------
// SERVER_EDITOR_FIELDS
// ---------------------------------------------------------------------------

#[test]
fn server_editor_fields_has_all_variants() {
    assert_eq!(SERVER_EDITOR_FIELDS.len(), 6);
    assert_eq!(SERVER_EDITOR_FIELDS[0], ServerEditorField::Name);
    assert_eq!(SERVER_EDITOR_FIELDS[5], ServerEditorField::Enabled);
}

// ---------------------------------------------------------------------------
// SourceDraft — construction and conversion
// ---------------------------------------------------------------------------

#[test]
fn source_draft_new_defaults() {
    let draft = SourceDraft::new();

    assert_eq!(draft.id, "");
    assert_eq!(draft.priority, "100");
    assert_eq!(draft.default_trust, "community");
    assert!(draft.enabled);
}

#[test]
fn source_draft_to_request_returns_none_when_id_empty() {
    let mut draft = SourceDraft::new();
    draft.display_name = "Test".to_string();
    draft.url = "http://example.com".to_string();
    assert!(draft.to_request().is_none());
}

#[test]
fn source_draft_to_request_returns_none_when_display_name_empty() {
    let mut draft = SourceDraft::new();
    draft.id = "test".to_string();
    draft.url = "http://example.com".to_string();
    assert!(draft.to_request().is_none());
}

#[test]
fn source_draft_to_request_returns_none_when_url_empty() {
    let mut draft = SourceDraft::new();
    draft.id = "test".to_string();
    draft.display_name = "Test".to_string();
    assert!(draft.to_request().is_none());
}

#[test]
fn source_draft_to_request_success() {
    let mut draft = SourceDraft::new();
    draft.id = "my-source".to_string();
    draft.display_name = "My Source".to_string();
    draft.url = "https://registry.example.com".to_string();
    draft.api_key_env = "MY_API_KEY".to_string();

    let req = draft.to_request().unwrap();
    assert_eq!(req.id, "my-source");
    assert_eq!(req.display_name, "My Source");
    assert_eq!(req.kind, "mcp_registry");
    assert_eq!(req.url, "https://registry.example.com");
    assert_eq!(req.api_key_env, Some("MY_API_KEY".to_string()));
    assert_eq!(req.priority, Some(100));
    assert_eq!(req.default_trust, Some("community".to_string()));
    assert_eq!(req.enabled, Some(true));
}

#[test]
fn source_draft_to_request_invalid_priority_defaults_to_100() {
    let mut draft = SourceDraft::new();
    draft.id = "s".to_string();
    draft.display_name = "S".to_string();
    draft.url = "http://x".to_string();
    draft.priority = "abc".to_string();

    let req = draft.to_request().unwrap();
    assert_eq!(req.priority, Some(100));
}

#[test]
fn source_draft_to_request_empty_api_key_env_becomes_none() {
    let mut draft = SourceDraft::new();
    draft.id = "s".to_string();
    draft.display_name = "S".to_string();
    draft.url = "http://x".to_string();
    draft.api_key_env = "  ".to_string();

    let req = draft.to_request().unwrap();
    assert_eq!(req.api_key_env, None);
}

// ---------------------------------------------------------------------------
// SourceDraft — field editing
// ---------------------------------------------------------------------------

#[test]
fn source_draft_push_char_all_fields() {
    let mut draft = SourceDraft::new();
    draft.push_char(SourceEditorField::Id, 'x');
    assert_eq!(draft.id, "x");

    draft.push_char(SourceEditorField::DisplayName, 'N');
    assert_eq!(draft.display_name, "N");

    draft.push_char(SourceEditorField::Url, 'h');
    assert_eq!(draft.url, "h");

    draft.push_char(SourceEditorField::ApiKeyEnv, 'K');
    assert_eq!(draft.api_key_env, "K");

    draft.push_char(SourceEditorField::DefaultTrust, 'v');
    assert!(draft.default_trust.ends_with('v'));
}

#[test]
fn source_draft_priority_rejects_non_digits() {
    let mut draft = SourceDraft::new();
    draft.priority.clear();
    draft.push_char(SourceEditorField::Priority, '5');
    draft.push_char(SourceEditorField::Priority, 'a');
    draft.push_char(SourceEditorField::Priority, '0');
    assert_eq!(draft.priority, "50");
}

#[test]
fn source_draft_enabled_toggle() {
    let mut draft = SourceDraft::new();
    assert!(draft.enabled);
    draft.push_char(SourceEditorField::Enabled, 'n');
    assert!(!draft.enabled);
    draft.push_char(SourceEditorField::Enabled, ' ');
    assert!(draft.enabled);
}

#[test]
fn source_draft_backspace_all_fields() {
    let mut draft = SourceDraft::new();
    draft.id = "ab".to_string();
    draft.backspace(SourceEditorField::Id);
    assert_eq!(draft.id, "a");

    draft.display_name = "xy".to_string();
    draft.backspace(SourceEditorField::DisplayName);
    assert_eq!(draft.display_name, "x");

    draft.url = "ht".to_string();
    draft.backspace(SourceEditorField::Url);
    assert_eq!(draft.url, "h");

    draft.backspace(SourceEditorField::Enabled);
}

#[test]
fn source_draft_clear_field() {
    let mut draft = SourceDraft::new();
    draft.id = "test".to_string();
    draft.clear_field(SourceEditorField::Id);
    assert_eq!(draft.id, "");

    draft.priority = "200".to_string();
    draft.clear_field(SourceEditorField::Priority);
    assert_eq!(draft.priority, "");

    draft.clear_field(SourceEditorField::Enabled);
}

// ---------------------------------------------------------------------------
// CatalogInstallDraft
// ---------------------------------------------------------------------------

#[test]
fn catalog_install_draft_new_is_empty() {
    let draft = CatalogInstallDraft::new();
    assert_eq!(draft.catalog_id, "");
    assert!(draft.items.is_empty());
    assert!(draft.values.is_empty());
}

#[test]
fn catalog_install_draft_from_entry_populates_fields() {
    let entry = make_server_entry();
    let draft = CatalogInstallDraft::from_entry(&entry);

    assert_eq!(draft.catalog_id, "test-catalog");
    assert_eq!(draft.source, "builtin");
    assert_eq!(draft.display_name, "Test Tool");
    assert_eq!(draft.items.len(), 1);
    assert_eq!(draft.items[0].key, "API_KEY");
    assert!(draft.items[0].required);
    assert!(draft.items[0].secret);
    assert_eq!(draft.values.get("API_KEY"), Some(&String::new()));
}

#[test]
fn catalog_install_draft_to_request_none_when_required_empty() {
    let entry = make_server_entry();
    let draft = CatalogInstallDraft::from_entry(&entry);

    assert!(draft.to_request().is_none());
}

#[test]
fn catalog_install_draft_to_request_success() {
    let entry = make_server_entry();
    let mut draft = CatalogInstallDraft::from_entry(&entry);
    draft
        .values
        .insert("API_KEY".to_string(), "sk-123".to_string());

    let req = draft.to_request().unwrap();
    assert_eq!(req.catalog_id, "test-catalog");
    assert_eq!(req.source, "builtin");
    assert!(!req.trust_grant);
    assert!(req.auto_start);
    assert_eq!(
        req.env_overrides.get("API_KEY"),
        Some(&"sk-123".to_string())
    );
}

#[test]
fn catalog_install_draft_push_char_and_backspace() {
    let entry = make_server_entry();
    let mut draft = CatalogInstallDraft::from_entry(&entry);

    draft.push_char(0, 's');
    draft.push_char(0, 'k');
    assert_eq!(draft.values.get("API_KEY"), Some(&"sk".to_string()));

    draft.backspace(0);
    assert_eq!(draft.values.get("API_KEY"), Some(&"s".to_string()));
}

#[test]
fn catalog_install_draft_clear_field() {
    let entry = make_server_entry();
    let mut draft = CatalogInstallDraft::from_entry(&entry);
    draft
        .values
        .insert("API_KEY".to_string(), "secret".to_string());

    draft.clear_field(0);
    assert_eq!(draft.values.get("API_KEY"), Some(&String::new()));
}

#[test]
fn catalog_install_draft_out_of_bounds_index_is_noop() {
    let entry = make_server_entry();
    let mut draft = CatalogInstallDraft::from_entry(&entry);
    draft.push_char(99, 'x');
    draft.backspace(99);
    draft.clear_field(99);
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

#[test]
fn split_args_basic() {
    assert_eq!(split_args("a b c"), vec!["a", "b", "c"]);
}

#[test]
fn split_args_extra_whitespace() {
    assert_eq!(split_args("  a   b  "), vec!["a", "b"]);
}

#[test]
fn split_args_empty() {
    assert!(split_args("").is_empty());
    assert!(split_args("   ").is_empty());
}

#[test]
fn trim_option_returns_none_for_empty() {
    assert_eq!(trim_option(""), None);
    assert_eq!(trim_option("   "), None);
}

#[test]
fn trim_option_returns_trimmed_value() {
    assert_eq!(trim_option("  hello  "), Some("hello".to_string()));
}

#[test]
fn parse_install_spec_valid_stdio() {
    let entry = make_server_entry();
    let spec = parse_install_spec(&entry);
    assert!(spec.is_some());
}

#[test]
fn parse_install_spec_invalid_json() {
    let mut entry = make_server_entry();
    entry.install_spec_json = "not json".to_string();
    assert!(parse_install_spec(&entry).is_none());
}

#[test]
fn parse_default_env_valid() {
    let entry = make_server_entry();
    let env = parse_default_env(&entry);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].key, "API_KEY");
}

#[test]
fn parse_default_env_invalid_json_returns_empty() {
    let mut entry = make_server_entry();
    entry.default_env_json = "bad".to_string();
    assert!(parse_default_env(&entry).is_empty());
}

#[test]
fn parse_requirements_empty() {
    let entry = make_server_entry();
    assert!(parse_requirements(&entry).is_empty());
}

#[test]
fn catalog_config_items_includes_env_vars() {
    let entry = make_server_entry();
    let items = catalog_config_items(&entry);
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i.key == "API_KEY" && i.kind == "env"));
}

#[test]
fn catalog_config_items_with_sse_headers() {
    let mut entry = make_server_entry();
    entry.install_spec_json =
        r#"{"transport":"sse","url":"http://x","headers":{"Authorization":"Bearer {API_KEY}"}}"#
            .to_string();
    entry.default_env_json = r#"[{"key":"Authorization","label":"Auth","description":"Auth header","required":true,"secret":true,"default":null}]"#.to_string();

    let items = catalog_config_items(&entry);
    assert!(items
        .iter()
        .any(|i| i.key == "Authorization" && i.kind == "HTTP header"));
    assert!(!items
        .iter()
        .any(|i| i.key == "Authorization" && i.kind == "env"));
}

#[test]
fn install_request_for_entry_builds_correct_request() {
    let entry = make_server_entry();
    let mut env = BTreeMap::new();
    env.insert("KEY".to_string(), "val".to_string());

    let req = install_request_for_entry(&entry, env);
    assert_eq!(req.catalog_id, "test-catalog");
    assert_eq!(req.source, "builtin");
    assert!(req.auto_start);
    assert!(!req.trust_grant);
    assert_eq!(req.env_overrides.get("KEY"), Some(&"val".to_string()));
}
