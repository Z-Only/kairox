use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SandboxPolicy {
    ReadOnly,
    WorkspaceWrite {
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        writable_roots: Vec<PathBuf>,
    },
    DangerFullAccess,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::WorkspaceWrite {
            network_access: false,
            writable_roots: Vec::new(),
        }
    }
}

impl SandboxPolicy {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::WorkspaceWrite { .. } => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }

    pub fn allows_network(&self) -> bool {
        matches!(
            self,
            Self::DangerFullAccess
                | Self::WorkspaceWrite {
                    network_access: true,
                    ..
                }
        )
    }

    /// Returns true when the policy permits writing the given path.
    /// For `ReadOnly` always false. For `DangerFullAccess` always true.
    /// For `WorkspaceWrite`, the path must canonicalize under the workspace
    /// root or one of the configured writable roots.
    pub fn path_writable(&self, path: &Path, workspace_root: &Path) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::DangerFullAccess => true,
            Self::WorkspaceWrite { writable_roots, .. } => {
                if path_under(path, workspace_root) {
                    return true;
                }
                writable_roots.iter().any(|root| path_under(path, root))
            }
        }
    }
}

fn path_under(path: &Path, root: &Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let root = root.components().collect::<Vec<_>>();
    if path.len() < root.len() {
        return false;
    }
    path.iter().zip(root.iter()).all(|(p, r)| p == r)
}

impl fmt::Display for SandboxPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind_str())
    }
}

impl std::str::FromStr for SandboxPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "read_only" | "readonly" => Ok(Self::ReadOnly),
            "workspace_write" | "workspacewrite" => Ok(Self::default()),
            "danger_full_access" | "dangerfullaccess" | "full_access" => {
                Ok(Self::DangerFullAccess)
            }
            other => Err(format!(
                "unknown sandbox policy `{other}`; expected read_only|workspace_write|danger_full_access"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
