use crate::{ModelClient, ModelEvent, ModelProfile, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ModelRouter {
    clients: HashMap<String, Arc<dyn ModelClient>>,
    profiles: HashMap<String, ModelProfile>,
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            profiles: HashMap::new(),
        }
    }

    pub fn register(&mut self, profile: ModelProfile, client: Arc<dyn ModelClient>) {
        let alias = profile.alias.clone();
        self.profiles.insert(alias.clone(), profile);
        self.clients.insert(alias, client);
    }

    pub fn get_profile(&self, alias: &str) -> Option<&ModelProfile> {
        self.profiles.get(alias)
    }

    pub fn list_profiles(&self) -> Vec<&ModelProfile> {
        let mut profiles: Vec<_> = self.profiles.values().collect();
        profiles.sort_by(|a, b| a.alias.cmp(&b.alias));
        profiles
    }

    /// Route a model request to the correct client by profile alias.
    /// This is the inherent method; the `ModelClient` trait impl delegates here.
    pub async fn route(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let client = self.clients.get(&request.model_profile).ok_or_else(|| {
            crate::ModelError::Request(format!("unknown model: '{}'", request.model_profile))
        })?;
        client.stream(request).await
    }
}

#[async_trait]
impl ModelClient for ModelRouter {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.route(request).await
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
