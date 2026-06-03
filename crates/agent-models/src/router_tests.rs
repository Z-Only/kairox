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
        .into_iter()
        .map(|p| p.alias)
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

#[test]
fn new_router_is_empty() {
    let router = ModelRouter::new();
    assert!(router.list_profiles().is_empty());
}

#[test]
fn register_and_list_single_profile() {
    let mut router = ModelRouter::new();
    router.register(
        test_profile("single"),
        Arc::new(FakeModelClient::new(vec![])),
    );
    assert_eq!(router.list_profiles().len(), 1);
    let profile = router.get_profile("single");
    assert!(profile.is_some());
    assert_eq!(profile.unwrap().alias, "single");
}

#[test]
fn register_and_list_multiple_sorted() {
    let mut router = ModelRouter::new();
    router.register(test_profile("c"), Arc::new(FakeModelClient::new(vec![])));
    router.register(test_profile("a"), Arc::new(FakeModelClient::new(vec![])));
    router.register(test_profile("b"), Arc::new(FakeModelClient::new(vec![])));
    let names: Vec<_> = router
        .list_profiles()
        .into_iter()
        .map(|p| p.alias)
        .collect();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[test]
fn get_profile_unknown_returns_none() {
    let router = ModelRouter::new();
    assert!(router.get_profile("nonexistent").is_none());
}

#[tokio::test]
async fn route_twice_uses_same_client() {
    let mut router = ModelRouter::new();
    let client = Arc::new(FakeModelClient::new(vec!["token".into()]));
    router.register(test_profile("test"), client);

    let mut stream1 = router
        .route(ModelRequest::user_text("test", "first"))
        .await
        .unwrap();
    let first = stream1.next().await.unwrap().unwrap();
    assert_eq!(first, ModelEvent::TokenDelta("token".into()));

    let mut stream2 = router
        .route(ModelRequest::user_text("test", "second"))
        .await
        .unwrap();
    let second = stream2.next().await.unwrap().unwrap();
    assert_eq!(second, ModelEvent::TokenDelta("token".into()));
}
