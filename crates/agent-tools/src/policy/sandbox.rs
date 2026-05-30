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
#[path = "sandbox_tests.rs"]
mod tests;
