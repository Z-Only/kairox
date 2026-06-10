use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ConfigScope {
    Builtin = 0,
    User = 1,
    Project = 2,
    Local = 3,
}

impl ConfigScope {
    pub fn priority(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            ConfigScope::Builtin => "builtin",
            ConfigScope::User => "user",
            ConfigScope::Project => "project",
            ConfigScope::Local => "local",
        }
    }
}

impl std::fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
#[path = "config_scope_tests.rs"]
mod tests;
