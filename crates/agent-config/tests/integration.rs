//! Integration tests for the agent-config crate.
//!
//! These tests exercise TOML parsing, router building, and `.kairox/` discovery
//! end-to-end through the public API.

use agent_config::{build_router, find_config_upward, load_from_str, resolve_api_keys, validate};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Test 1: parse minimal config
// ---------------------------------------------------------------------------

#[test]
fn parse_minimal_config() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
"#;
    let config = load_from_str(toml, "test.toml").expect("should parse minimal config");
    assert_eq!(config.profiles.len(), 1);
    let (alias, def) = &config.profiles[0];
    assert_eq!(alias, "fake");
    assert_eq!(def.provider, "fake");
}

// ---------------------------------------------------------------------------
// Test 2: build router from profiles
// ---------------------------------------------------------------------------

#[test]
fn build_router_from_profiles() {
    let toml = r#"
[profiles.fake-one]
provider = "fake"
model_id = "fake"
response = "response from one"

[profiles.fake-two]
provider = "fake"
model_id = "fake"
response = "response from two"
"#;
    let mut config = load_from_str(toml, "test.toml").expect("should parse two profiles");
    resolve_api_keys(&mut config);
    validate(&config).expect("config should be valid");

    let router = build_router(&config);
    let profiles = router.list_profiles();
    assert_eq!(profiles.len(), 2);
    let aliases: Vec<&str> = profiles.iter().map(|p| p.alias.as_str()).collect();
    assert!(aliases.contains(&"fake-one"));
    assert!(aliases.contains(&"fake-two"));
}

// ---------------------------------------------------------------------------
// Test 3: discover project-local .kairox/config.toml
// ---------------------------------------------------------------------------

#[test]
fn discovers_project_local_config() {
    let dir = TempDir::new().expect("create temp dir");

    // Create .kairox/config.toml inside the temp dir
    let config_dir = dir.path().join(".kairox");
    std::fs::create_dir_all(&config_dir).expect("create .kairox dir");

    let config_path = config_dir.join("config.toml");
    let config_content = "[profiles.my-profile]\nprovider = \"fake\"\nmodel_id = \"fake\"\n";
    std::fs::write(&config_path, config_content).expect("write config.toml");

    // Discover upward from the temp dir itself
    let (found_path, source) =
        find_config_upward(dir.path()).expect("should discover .kairox/config.toml");

    assert_eq!(found_path, config_path);
    assert!(matches!(source, agent_config::ConfigSource::ProjectFile));

    // Verify file content contains the expected profile name
    let content = std::fs::read_to_string(&found_path).expect("read discovered config");
    assert!(content.contains("my-profile"));
}
