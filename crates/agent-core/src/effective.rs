use serde::{Deserialize, Serialize};

use crate::config_scope::ConfigScope;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + serde::de::DeserializeOwned")]
pub struct EffectiveItem<T: Serialize + serde::de::DeserializeOwned + Clone> {
    pub value: T,
    pub source: ConfigScope,
    pub overrides: Option<ConfigScope>,
    pub enabled: bool,
    pub disabled_by: Option<ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl<T: Serialize + serde::de::DeserializeOwned + Clone> EffectiveItem<T> {
    pub fn new(value: T, source: ConfigScope) -> Self {
        Self {
            value,
            source,
            overrides: None,
            enabled: true,
            disabled_by: None,
            writable: source >= ConfigScope::User,
            deletable: source >= ConfigScope::User,
        }
    }

    pub fn with_disabled(mut self, by: ConfigScope) -> Self {
        self.enabled = false;
        self.disabled_by = Some(by);
        self
    }

    pub fn with_override(mut self, by: ConfigScope) -> Self {
        self.overrides = Some(by);
        self
    }
}

#[cfg(test)]
#[path = "effective_tests.rs"]
mod tests;
