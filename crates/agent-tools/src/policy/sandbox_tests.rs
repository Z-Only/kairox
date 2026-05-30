use super::*;

#[test]
fn default_is_workspace_write_no_net() {
    let p = SandboxPolicy::default();
    assert_eq!(p.kind_str(), "workspace_write");
    assert!(!p.allows_network());
}

#[test]
fn read_only_blocks_everything() {
    let p = SandboxPolicy::ReadOnly;
    assert!(!p.allows_network());
    assert!(!p.path_writable(Path::new("/ws/foo"), Path::new("/ws")));
}

#[test]
fn workspace_write_path_inside_root() {
    let p = SandboxPolicy::WorkspaceWrite {
        network_access: false,
        writable_roots: vec![],
    };
    assert!(p.path_writable(Path::new("/ws/foo"), Path::new("/ws")));
    assert!(!p.path_writable(Path::new("/elsewhere/bar"), Path::new("/ws")));
}

#[test]
fn workspace_write_extra_roots() {
    let p = SandboxPolicy::WorkspaceWrite {
        network_access: true,
        writable_roots: vec![PathBuf::from("/tmp/extra")],
    };
    assert!(p.allows_network());
    assert!(p.path_writable(Path::new("/tmp/extra/x.log"), Path::new("/ws")));
    assert!(!p.path_writable(Path::new("/other"), Path::new("/ws")));
}

#[test]
fn danger_writes_everywhere_and_allows_network() {
    let p = SandboxPolicy::DangerFullAccess;
    assert!(p.allows_network());
    assert!(p.path_writable(Path::new("/anywhere"), Path::new("/ws")));
}

#[test]
fn serde_workspace_write_with_defaults() {
    let json = r#"{"kind":"workspace_write"}"#;
    let p: SandboxPolicy = serde_json::from_str(json).unwrap();
    assert_eq!(
        p,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![]
        }
    );
}

#[test]
fn serde_read_only_and_danger() {
    let r: SandboxPolicy = serde_json::from_str(r#"{"kind":"read_only"}"#).unwrap();
    assert_eq!(r, SandboxPolicy::ReadOnly);
    let d: SandboxPolicy = serde_json::from_str(r#"{"kind":"danger_full_access"}"#).unwrap();
    assert_eq!(d, SandboxPolicy::DangerFullAccess);
}

#[test]
fn serde_workspace_write_with_fields() {
    let p = SandboxPolicy::WorkspaceWrite {
        network_access: true,
        writable_roots: vec![PathBuf::from("/tmp")],
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: SandboxPolicy = serde_json::from_str(&s).unwrap();
    assert_eq!(back, p);
}

#[test]
fn fromstr_canonical() {
    assert_eq!(
        "read_only".parse::<SandboxPolicy>().unwrap(),
        SandboxPolicy::ReadOnly
    );
    assert_eq!(
        "workspace_write"
            .parse::<SandboxPolicy>()
            .unwrap()
            .kind_str(),
        "workspace_write"
    );
    assert_eq!(
        "danger_full_access".parse::<SandboxPolicy>().unwrap(),
        SandboxPolicy::DangerFullAccess
    );
}

#[test]
fn fromstr_aliases() {
    assert_eq!(
        "ReadOnly".parse::<SandboxPolicy>().unwrap(),
        SandboxPolicy::ReadOnly
    );
    assert_eq!(
        "read-only".parse::<SandboxPolicy>().unwrap(),
        SandboxPolicy::ReadOnly
    );
    assert_eq!(
        "full_access".parse::<SandboxPolicy>().unwrap(),
        SandboxPolicy::DangerFullAccess
    );
}

#[test]
fn fromstr_invalid() {
    assert!("bogus".parse::<SandboxPolicy>().is_err());
}
