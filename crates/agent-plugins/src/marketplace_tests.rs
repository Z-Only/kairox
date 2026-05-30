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
