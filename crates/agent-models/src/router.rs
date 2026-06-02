use crate::{ModelClient, ModelEvent, ModelProfile, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub struct ModelRouter {
    inner: RwLock<ModelRouterInner>,
}

struct ModelRouterInner {
    clients: HashMap<String, Arc<dyn ModelClient>>,
    profiles: HashMap<String, ModelProfile>,
}

impl ModelRouterInner {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
            profiles: HashMap::new(),
        }
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(ModelRouterInner::new()),
        }
    }

    pub fn register(&mut self, profile: ModelProfile, client: Arc<dyn ModelClient>) {
        self.register_inner(profile, client);
    }

    pub fn replace_with(&self, other: ModelRouter) {
        let other_inner = other
            .inner
            .into_inner()
            .expect("model router lock should not be poisoned");
        *self
            .inner
            .write()
            .expect("model router lock should not be poisoned") = other_inner;
    }

    fn register_inner(&self, profile: ModelProfile, client: Arc<dyn ModelClient>) {
        let alias = profile.alias.clone();
        let mut inner = self
            .inner
            .write()
            .expect("model router lock should not be poisoned");
        inner.profiles.insert(alias.clone(), profile);
        inner.clients.insert(alias, client);
    }

    pub fn get_profile(&self, alias: &str) -> Option<ModelProfile> {
        self.inner
            .read()
            .expect("model router lock should not be poisoned")
            .profiles
            .get(alias)
            .cloned()
    }

    pub fn list_profiles(&self) -> Vec<ModelProfile> {
        let mut profiles: Vec<_> = self
            .inner
            .read()
            .expect("model router lock should not be poisoned")
            .profiles
            .values()
            .cloned()
            .collect();
        profiles.sort_by(|a, b| a.alias.cmp(&b.alias));
        profiles
    }

    /// Route a model request to the correct client by profile alias.
    /// This is the inherent method; the `ModelClient` trait impl delegates here.
    pub async fn route(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let client = {
            let inner = self
                .inner
                .read()
                .expect("model router lock should not be poisoned");
            inner
                .clients
                .get(&request.model_profile)
                .cloned()
                .ok_or_else(|| {
                    crate::ModelError::Request(format!(
                        "unknown model: '{}'",
                        request.model_profile
                    ))
                })?
        };
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
