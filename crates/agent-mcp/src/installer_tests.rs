use super::*;
use crate::catalog::{RuntimeKind, TrustLevel};

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
        id: "test-server".into(),
        source: "builtin".into(),
        display_name: "Test Server".into(),
        summary: "A test server".into(),
        description: "A server for testing installer behaviour.".into(),
        categories: vec!["test".into()],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        install: InstallSpec::Stdio {
            command: "echo".into(),
            args: vec!["hello".into()],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![],
        trust: TrustLevel::Community,
        default_env: vec![],
        icon: None,
        verified: false,
    }
}

fn install_request(entry: &ServerEntry) -> InstallRequest {
    InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    }
}

#[test]
fn build_section_env_output_is_valid_toml() {
    let entry = ServerEntry {
        id: "git".into(),
        source: "builtin".into(),
        display_name: "Git".into(),
        summary: "...".into(),
        description: "...".into(),
        categories: vec![],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        install: InstallSpec::Stdio {
            command: "uvx".into(),
            args: vec!["mcp-server-git".into(), "--repository".into(), ".".into()],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![],
        trust: TrustLevel::Verified,
        default_env: vec![EnvVarSpec {
            key: "REPO_PATH".into(),
            label: "Repository path".into(),
            description: "Path to a git repository on disk.".into(),
            required: true,
            secret: false,
            default: Some(".".into()),
        }],
        icon: None,
        verified: true,
    };
    let req = InstallRequest {
        catalog_id: "git".into(),
        source: "builtin".into(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let resolved = resolve_env(&entry.default_env, &req.env_overrides).unwrap();
    let section = build_section(&entry, &resolved);
    // Render the exact same way the real install() method does.
    let mut doc = toml_edit::DocumentMut::new();
    ensure_table(&mut doc, "mcp_servers");
    doc["mcp_servers"]["git"] = toml_edit::Item::Table(section);
    let rendered = doc.to_string();
    eprintln!("=== build_section output ===\n{rendered}===");
    assert!(
        rendered.contains("env = {"),
        "env must be an inline table, got:\n{rendered}"
    );
    let parsed: toml::value::Table =
        toml::from_str(&rendered).expect("build_section must produce valid TOML");
    // Spot-check that mcp_servers.git.env.REPO_PATH = "."
    let servers = parsed["mcp_servers"].as_table().unwrap();
    let git_srv = servers["git"].as_table().unwrap();
    let env = git_srv["env"].as_table().unwrap();
    assert_eq!(env["REPO_PATH"].as_str(), Some("."));
}

#[test]
fn installer_new_is_not_installed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("config.toml");
    let probe = Arc::new(StaticProbe { available: vec![] });
    let installer = Installer::new(toml_path, probe);

    let ids = installer
        .list_installed_ids()
        .expect("list_installed_ids should succeed");
    assert!(
        ids.is_empty(),
        "new installer should report zero installed servers"
    );
}

#[tokio::test]
async fn installer_can_start_install() {
    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("config.toml");
    let probe = Arc::new(StaticProbe { available: vec![] });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = install_request(&entry);

    // Before install, no servers installed.
    let before = installer.list_installed_ids().unwrap();
    assert!(
        before.is_empty(),
        "should have no installed servers before install"
    );

    // Install the entry (no runtime requirements, so it should succeed).
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(
        matches!(outcome, InstallOutcomeView::Installed { ref started, .. } if !started),
        "expected Installed with started=false, got {:?}",
        outcome
    );

    // After install, the server should appear in the list.
    let after = installer.list_installed_ids().unwrap();
    assert_eq!(after, vec!["test-server"]);

    // The TOML file should now exist.
    assert!(toml_path.exists(), "TOML file should be created on install");

    // Verify the file contains valid TOML that both toml_edit and toml can parse.
    let raw = std::fs::read_to_string(&toml_path).unwrap();
    eprintln!("=== installer output ===\n{raw}===");
    assert!(
        !raw.contains("Managed by Kairox marketplace"),
        "installer must not mark the unified config.toml as marketplace-owned"
    );
    let _doc = raw
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|e| panic!("installer output must be valid toml_edit: {e}\n{raw}"));
    toml::from_str::<toml::value::Table>(&raw)
        .unwrap_or_else(|e| panic!("installer output must be valid toml: {e}\n{raw}"));
}

#[tokio::test]
async fn installer_lists_catalog_metadata_for_overridden_server_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("config.toml");
    let probe = Arc::new(StaticProbe { available: vec![] });
    let installer = Installer::new(toml_path, probe);

    let entry = sample_entry();
    let mut req = install_request(&entry);
    req.server_id_override = Some("custom-server-id".into());
    installer.install(&entry, &req).await.unwrap();

    let records = installer.list_installed_records().unwrap();
    assert_eq!(
        records,
        vec![InstalledServerRecord {
            server_id: "custom-server-id".into(),
            catalog_id: Some("test-server".into()),
            source: Some("builtin".into()),
        }]
    );
}
