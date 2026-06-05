use super::*;

#[test]
fn parses_claude_marketplace_json() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "my-plugins",
          "owner": {"name": "Team"},
          "plugins": [
            {
              "name": "quality-review",
              "source": "./plugins/quality-review",
              "description": "Review code",
              "version": "1.0.0",
              "homepage": "https://example.com/quality-review",
              "repository": {"url": "https://github.com/example/quality-review"},
              "keywords": ["review", "coding"],
              "category": "Coding",
              "trust": "verified"
            }
          ]
        }"#,
    )
    .expect("marketplace");

    assert_eq!(marketplace.name, "my-plugins");
    assert_eq!(marketplace.plugins.len(), 1);
    assert_eq!(marketplace.plugins[0].source, "./plugins/quality-review");
    assert_eq!(
        marketplace.plugins[0].homepage.as_deref(),
        Some("https://example.com/quality-review")
    );
    assert_eq!(
        marketplace.plugins[0].repository.as_deref(),
        Some("https://github.com/example/quality-review")
    );
    assert_eq!(marketplace.plugins[0].keywords, vec!["review", "coding"]);
    assert_eq!(marketplace.plugins[0].category.as_deref(), Some("Coding"));
    assert_eq!(marketplace.plugins[0].trust.as_deref(), Some("verified"));
}

#[test]
fn parses_marketplace_with_no_plugins() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "empty-catalog",
          "plugins": []
        }"#,
    )
    .expect("marketplace");

    assert_eq!(marketplace.name, "empty-catalog");
    assert_eq!(marketplace.display_name, "empty-catalog");
    assert!(marketplace.plugins.is_empty());
}

#[test]
fn parses_marketplace_with_display_name() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "my-catalog",
          "display_name": "My Custom Catalog",
          "plugins": []
        }"#,
    )
    .expect("marketplace");

    assert_eq!(marketplace.name, "my-catalog");
    assert_eq!(marketplace.display_name, "My Custom Catalog");
}

#[test]
fn parses_plugin_with_string_source() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "test",
          "plugins": [
            {
              "name": "simple-plugin",
              "source": "./path/to/plugin"
            }
          ]
        }"#,
    )
    .expect("marketplace");

    assert_eq!(marketplace.plugins[0].source, "./path/to/plugin");
}

#[test]
fn parses_plugin_with_object_source() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "test",
          "plugins": [
            {
              "name": "remote-plugin",
              "source": {"type": "git", "url": "https://github.com/example/plugin.git"}
            }
          ]
        }"#,
    )
    .expect("marketplace");

    let source = &marketplace.plugins[0].source;
    assert!(source.contains("\"type\":\"git\""));
    assert!(source.contains("\"url\":\"https://github.com/example/plugin.git\""));
}

#[test]
fn parses_plugin_with_string_repository() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "test",
          "plugins": [
            {
              "name": "repo-plugin",
              "source": "./local",
              "repository": "https://github.com/example/repo"
            }
          ]
        }"#,
    )
    .expect("marketplace");

    assert_eq!(
        marketplace.plugins[0].repository.as_deref(),
        Some("https://github.com/example/repo")
    );
}

#[test]
fn parses_plugin_with_missing_optional_fields() {
    let marketplace = parse_marketplace(
        r#"{
          "name": "test",
          "plugins": [
            {
              "name": "minimal-plugin",
              "source": "./minimal"
            }
          ]
        }"#,
    )
    .expect("marketplace");

    let plugin = &marketplace.plugins[0];
    assert_eq!(plugin.name, "minimal-plugin");
    assert_eq!(plugin.source, "./minimal");
    assert_eq!(plugin.description, "");
    assert_eq!(plugin.version, None);
    assert_eq!(plugin.homepage, None);
    assert_eq!(plugin.repository, None);
    assert!(plugin.keywords.is_empty());
    assert_eq!(plugin.category, None);
    assert_eq!(plugin.trust, None);
}

#[test]
fn rejects_invalid_json() {
    let result = parse_marketplace(r#"{ not valid json }"#);
    assert!(result.is_err());
}
