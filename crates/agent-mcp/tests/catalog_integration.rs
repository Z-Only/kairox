use agent_mcp::catalog::{
    BuiltinCatalogProvider, CatalogProvider, CatalogQuery, InstallSpec, RuntimeKind, ServerEntry,
    TrustLevel,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load the builtin catalog provider (panics if the embedded catalog is malformed).
fn load_builtin() -> BuiltinCatalogProvider {
    BuiltinCatalogProvider::new().expect("builtin catalog must be well-formed")
}

// ---------------------------------------------------------------------------
// 1. builtin_catalog_is_well_formed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn builtin_catalog_is_well_formed() {
    let provider = load_builtin();
    let entries = provider
        .list(&CatalogQuery::default())
        .await
        .expect("list should succeed");

    assert!(!entries.is_empty(), "builtin catalog must contain entries");

    for entry in &entries {
        // --- source ---
        assert_eq!(
            entry.source, "builtin",
            "entry '{}': source must be 'builtin'",
            entry.id
        );

        // --- trust ---
        // TrustLevel is a valid enum value (enforced by deserialization).

        // --- categories ---
        assert!(
            !entry.categories.is_empty(),
            "entry '{}': categories must not be empty",
            entry.id
        );
        for cat in &entry.categories {
            assert!(
                !cat.is_empty(),
                "entry '{}': category must not be empty",
                entry.id
            );
        }

        // --- install spec ---
        match &entry.install {
            InstallSpec::Stdio {
                command,
                args: _,
                env: _,
                cwd: _,
            } => {
                assert!(
                    !command.is_empty(),
                    "entry '{}': stdio command must not be empty",
                    entry.id
                );
            }
            InstallSpec::Sse { url, headers: _ } => {
                assert!(
                    !url.is_empty(),
                    "entry '{}': SSE url must not be empty",
                    entry.id
                );
            }
        }

        // --- runtime requirements ---
        for req in &entry.requirements {
            // RuntimeKind is a valid enum (enforced by deserialization).
            let _kind_str = req.kind.as_str();
            if let Some(ref min_ver) = req.min_version {
                assert!(
                    !min_ver.is_empty(),
                    "entry '{}': min_version must not be empty if present",
                    entry.id
                );
            }
        }

        // --- env var specs ---
        for spec in &entry.default_env {
            assert!(
                !spec.key.is_empty(),
                "entry '{}': env var key must not be empty",
                entry.id
            );
            assert!(
                !spec.label.is_empty(),
                "entry '{}': env var '{}' label must not be empty",
                entry.id,
                spec.key
            );
            assert!(
                !spec.description.is_empty(),
                "entry '{}': env var '{}' description must not be empty",
                entry.id,
                spec.key
            );
            if spec.required && spec.default.is_none() {
                // Required env vars without a default must be documented — this
                // is fine as long as the label/description are present (already
                // checked above).
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. catalog_entry_deserialization
// ---------------------------------------------------------------------------

#[test]
fn catalog_entry_deserialization_stdio() {
    let json = json!({
        "id": "my-server",
        "source": "builtin",
        "display_name": "My Server",
        "summary": "A short summary.",
        "description": "Longer description of the server.",
        "categories": ["dev-tools"],
        "tags": ["testing"],
        "author": "Test Author",
        "homepage": "https://example.com",
        "version": "1.2.3",
        "install": {
            "transport": "stdio",
            "command": "my-cmd",
            "args": ["--flag", "${CONFIG_PATH}"],
            "env": { "EXTRA": "value" },
            "cwd": "/tmp"
        },
        "requirements": [
            { "kind": "node", "min_version": ">=18.0.0", "install_hint": "https://nodejs.org" }
        ],
        "trust": "verified",
        "default_env": [
            {
                "key": "CONFIG_PATH",
                "label": "Config Path",
                "description": "Path to the configuration file.",
                "required": true,
                "secret": false,
                "default": "/etc/config"
            }
        ],
        "icon": "icon.png"
    });

    let entry: ServerEntry = serde_json::from_value(json).expect("deserialization should succeed");

    assert_eq!(entry.id, "my-server");
    assert_eq!(entry.source, "builtin");
    assert_eq!(entry.display_name, "My Server");
    assert_eq!(entry.summary, "A short summary.");
    assert_eq!(entry.description, "Longer description of the server.");
    assert_eq!(entry.categories, vec!["dev-tools"]);
    assert_eq!(entry.tags, vec!["testing"]);
    assert_eq!(entry.author.as_deref(), Some("Test Author"));
    assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
    assert_eq!(entry.version.as_deref(), Some("1.2.3"));
    assert_eq!(entry.trust, TrustLevel::Verified);
    assert_eq!(entry.icon.as_deref(), Some("icon.png"));

    match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            assert_eq!(command, "my-cmd");
            assert_eq!(
                *args,
                vec!["--flag".to_string(), "${CONFIG_PATH}".to_string()]
            );
            assert_eq!(env.get("EXTRA").map(String::as_str), Some("value"));
            assert_eq!(cwd.as_deref(), Some("/tmp"));
        }
        other => panic!("expected Stdio install spec, got {other:?}"),
    }

    assert_eq!(entry.requirements.len(), 1);
    assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
    assert_eq!(
        entry.requirements[0].min_version.as_deref(),
        Some(">=18.0.0")
    );

    assert_eq!(entry.default_env.len(), 1);
    assert_eq!(entry.default_env[0].key, "CONFIG_PATH");
    assert_eq!(entry.default_env[0].required, true);
    assert_eq!(entry.default_env[0].default.as_deref(), Some("/etc/config"));
}

#[test]
fn catalog_entry_deserialization_sse() {
    let json = json!({
        "id": "sse-server",
        "source": "community",
        "display_name": "SSE Server",
        "summary": "An SSE-based MCP server.",
        "description": "Communicates over SSE + HTTP POST.",
        "categories": ["remote"],
        "tags": ["http", "sse"],
        "install": {
            "transport": "sse",
            "url": "https://mcp.example.com",
            "headers": {
                "Authorization": "Bearer secret-token",
                "X-Request-Id": "abc123"
            }
        },
        "requirements": [],
        "trust": "community",
        "default_env": [],
        "icon": null
    });

    let entry: ServerEntry = serde_json::from_value(json).expect("deserialization should succeed");

    assert_eq!(entry.id, "sse-server");
    assert_eq!(entry.source, "community");
    assert_eq!(entry.trust, TrustLevel::Community);

    match &entry.install {
        InstallSpec::Sse { url, headers } => {
            assert_eq!(url, "https://mcp.example.com");
            assert_eq!(headers.len(), 2);
            assert_eq!(
                headers.get("Authorization").map(String::as_str),
                Some("Bearer secret-token")
            );
            assert_eq!(
                headers.get("X-Request-Id").map(String::as_str),
                Some("abc123")
            );
        }
        other => panic!("expected Sse install spec, got {other:?}"),
    }
}

#[test]
fn catalog_entry_deserialization_minimal_fields() {
    // A minimal entry with only required fields (summary is just description
    // renamed in our schema but both are checked separately).
    let json = json!({
        "id": "minimal",
        "source": "custom",
        "display_name": "Minimal",
        "summary": "Minimal summary.",
        "description": "Minimal description.",
        "categories": [],
        "tags": [],
        "install": {
            "transport": "stdio",
            "command": "cmd",
            "args": []
        },
        "requirements": [],
        "trust": "unverified",
        "default_env": [],
        "icon": null
    });

    let entry: ServerEntry =
        serde_json::from_value(json).expect("minimal entry should deserialize");

    assert_eq!(entry.id, "minimal");
    assert_eq!(entry.source, "custom");
    assert_eq!(entry.trust, TrustLevel::Unverified);
    assert!(entry.author.is_none());
    assert!(entry.homepage.is_none());
    assert!(entry.version.is_none());

    match &entry.install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "cmd");
            assert!(args.is_empty());
        }
        other => panic!("expected Stdio, got {other:?}"),
    }
}

#[test]
fn catalog_entry_deserialization_unknown_transport_errors() {
    // An install spec with an unknown transport tag should fail deserialization.
    let json = json!({
        "id": "bad-transport",
        "source": "builtin",
        "display_name": "Bad Transport",
        "summary": "s",
        "description": "d",
        "categories": [],
        "tags": [],
        "install": {
            "transport": "websocket",
            "command": "cmd",
            "args": []
        },
        "requirements": [],
        "trust": "unverified",
        "default_env": [],
        "icon": null
    });

    let result: Result<ServerEntry, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "unknown transport tag should cause deserialization error"
    );
}

// ---------------------------------------------------------------------------
// 3. catalog_search_by_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn catalog_search_by_id_known_entries() {
    let provider = load_builtin();

    // Verify known entries can be found.
    let known_ids = [
        "filesystem",
        "github",
        "postgres",
        "sqlite",
        "brave-search",
        "puppeteer",
        "memory",
        "git",
        "fetch",
    ];

    for id in &known_ids {
        let found = provider
            .get(id)
            .await
            .unwrap_or_else(|e| panic!("get('{id}') should succeed: {e}"));
        assert!(
            found.is_some(),
            "known entry '{id}' should be found in builtin catalog"
        );
        let entry = found.unwrap();
        assert_eq!(entry.id, *id, "returned entry id should match queried id");
    }
}

#[tokio::test]
async fn catalog_search_by_id_unknown_returns_none() {
    let provider = load_builtin();

    let unknown_ids = ["does-not-exist", "nonexistent-server-xyz", "", "  "];

    for id in &unknown_ids {
        let result = provider.get(id).await.expect("get should not error");
        assert!(result.is_none(), "unknown id '{id}' should return None");
    }
}

#[tokio::test]
async fn catalog_search_by_id_exact_match_only() {
    let provider = load_builtin();

    // Partial matches should NOT return results (get is exact match only).
    // "git" is a real entry, so verify it separately below.
    // "file" is not an exact ID (only "filesystem" exists), and "post" is
    // not an exact ID (only "postgres" exists).
    let git_entry = provider.get("git").await.unwrap();
    assert!(git_entry.is_some(), "'git' should be a real entry");

    // "file" and "post" should return None — they're not exact IDs.
    for id in ["file", "post"] {
        let result = provider.get(id).await.unwrap();
        assert!(
            result.is_none(),
            "partial match '{id}' should return None for exact get()"
        );
    }
}
