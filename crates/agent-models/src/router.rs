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
            crate::ModelError::Request(format!(
                "unknown model profile: '{}'",
                request.model_profile
            ))
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
mod tests {
    use super::*;
    use crate::{FakeModelClient, ModelCapabilities};
    use futures::StreamExt;

    fn test_profile(alias: &str) -> ModelProfile {
        ModelProfile {
            alias: alias.into(),
            provider: "fake".into(),
            model_id: "test".into(),
            capabilities: ModelCapabilities {
                streaming: true,
                tool_calling: false,
                json_schema: false,
                vision: false,
                reasoning_controls: false,
                context_window: 4096,
                output_limit: 2048,
                local_model: true,
            },
        }
    }

    #[tokio::test]
    async fn routes_to_correct_client_by_profile_alias() {
        let mut router = ModelRouter::new();
        let fast_client = Arc::new(FakeModelClient::new(vec!["fast response".into()]));
        let deep_client = Arc::new(FakeModelClient::new(vec!["deep response".into()]));

        router.register(test_profile("fast"), fast_client);
        router.register(test_profile("deep-reasoning"), deep_client);

        let mut stream = router
            .route(ModelRequest::user_text("fast", "hello"))
            .await
            .unwrap();

        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first, ModelEvent::TokenDelta("fast response".into()));
    }

    #[tokio::test]
    async fn returns_error_for_unknown_profile() {
        let router = ModelRouter::new();
        let result = router
            .route(ModelRequest::user_text("nonexistent", "hello"))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn lists_registered_profiles_sorted() {
        let mut router = ModelRouter::new();
        router.register(
            test_profile("deep-reasoning"),
            Arc::new(FakeModelClient::new(vec![])),
        );
        router.register(test_profile("fast"), Arc::new(FakeModelClient::new(vec![])));

        let names: Vec<_> = router
            .list_profiles()
            .iter()
            .map(|p| p.alias.as_str())
            .collect();
        assert_eq!(names, vec!["deep-reasoning", "fast"]);
    }

    #[tokio::test]
    async fn model_client_trait_impl_delegates_to_route() {
        let mut router = ModelRouter::new();
        let client = Arc::new(FakeModelClient::new(vec!["trait response".into()]));
        router.register(test_profile("test"), client);

        // Call via ModelClient trait
        let mut stream = router
            .stream(ModelRequest::user_text("test", "hello"))
            .await
            .unwrap();

        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first, ModelEvent::TokenDelta("trait response".into()));
    }
}
