use agent_mcp::catalog::{
    EnvVarSpec, InstallRequest, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry,
    TrustLevel,
};
use agent_mcp::installer::{InstallOutcomeView, Installer, RuntimeProbe};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

/// Deterministic probe for tests.
struct StaticProbe {
    available: Vec<RuntimeKind>,
}

#[async_trait::async_trait]
impl RuntimeProbe for StaticProbe {
    async fn is_available(&self, kind: RuntimeKind) -> bool {
        self.available.contains(&kind)
    }
}

fn sample_entry() -> ServerEntry {
    ServerEntry {
        id: "filesystem".into(),
        source: "builtin".into(),
        display_name: "Filesystem".into(),
        summary: "s".into(),
        description: "d".into(),
        categories: vec!["filesystem".into()],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        install: InstallSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "pkg".into(), "${WORKSPACE_PATH}".into()],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![RuntimeRequirement {
            kind: RuntimeKind::Node,
            min_version: None,
            install_hint: Some("https://nodejs.org".into()),
        }],
        trust: TrustLevel::Verified,
        default_env: vec![EnvVarSpec {
            key: "WORKSPACE_PATH".into(),
            label: "Workspace path".into(),
            description: "".into(),
            required: true,
            secret: false,
            default: Some("/tmp/x".into()),
        }],
        icon: None,
        verified: false,
    }
}

#[tokio::test]
async fn install_writes_toml_and_marks_trust() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe {
        available: vec![RuntimeKind::Node],
    });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: true,
        auto_start: true,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::Installed { .. }));

    let body = std::fs::read_to_string(&toml_path).unwrap();
    assert!(body.contains("[mcp_servers.filesystem]"));
    assert!(
        body.contains("\"/tmp/x\""),
        "VAR substitution must materialize"
    );
    assert!(body.contains("trusted_servers"));
    assert!(body.contains("\"filesystem\""));
}

#[tokio::test]
async fn install_runtime_missing_does_not_write_toml() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![] });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::RuntimeMissing { .. }));
    assert!(!toml_path.exists());
}

#[tokio::test]
async fn install_invalid_env_when_required_missing() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe {
        available: vec![RuntimeKind::Node],
    });
    let installer = Installer::new(toml_path, probe);

    let mut entry = sample_entry();
    entry.default_env[0].default = None; // required, no default, no override → invalid
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::InvalidEnv { .. }));
}

#[tokio::test]
async fn install_id_collision_returns_already_installed() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe {
        available: vec![RuntimeKind::Node],
    });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: true,
    };
    installer.install(&entry, &req).await.unwrap();
    let again = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(again, InstallOutcomeView::AlreadyInstalled { .. }));
}

#[tokio::test]
async fn uninstall_removes_section_and_trust() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe {
        available: vec![RuntimeKind::Node],
    });
    let installer = Installer::new(toml_path.clone(), probe);
    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: true,
        auto_start: false,
    };
    installer.install(&entry, &req).await.unwrap();

    installer.uninstall("filesystem").await.unwrap();
    let body = std::fs::read_to_string(&toml_path).unwrap_or_default();
    assert!(!body.contains("[mcp_servers.filesystem]"));
}
