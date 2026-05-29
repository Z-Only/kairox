use super::*;
use crate::transport::Transport;
use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpError};
use async_trait::async_trait;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Mutex as StdMutex;

/// Shared inner state for the mock transport.
#[derive(Debug, Default)]
struct MockState {
    responses: VecDeque<JsonRpcResponse>,
    notifications: Vec<JsonRpcNotification>,
    requests: Vec<JsonRpcRequest>,
}

struct MockTransport {
    state: Arc<StdMutex<MockState>>,
}

impl MockTransport {
    fn new(state: Arc<StdMutex<MockState>>) -> Self {
        Self { state }
    }

    fn enqueue_response(state: &Arc<StdMutex<MockState>>, response: JsonRpcResponse) {
        state.lock().unwrap().responses.push_back(response);
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> crate::Result<JsonRpcResponse> {
        self.state.lock().unwrap().requests.push(request);
        self.state
            .lock()
            .unwrap()
            .responses
            .pop_front()
            .ok_or_else(|| McpError::Transport("no response queued".into()))
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> crate::Result<()> {
        self.state.lock().unwrap().notifications.push(notification);
        Ok(())
    }

    async fn close(&mut self) -> crate::Result<()> {
        Ok(())
    }
}

fn make_tools_response(tools: &[&str]) -> JsonRpcResponse {
    let tools_json: Vec<serde_json::Value> =
        tools.iter().map(|name| json!({ "name": name })).collect();
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: json!({ "tools": tools_json }),
    }
}

fn make_resources_response(uris: &[&str]) -> JsonRpcResponse {
    let resources_json: Vec<serde_json::Value> = uris
        .iter()
        .map(|uri| json!({ "uri": uri, "name": uri }))
        .collect();
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: json!({ "resources": resources_json }),
    }
}

fn make_prompts_response(names: &[&str]) -> JsonRpcResponse {
    let prompts_json: Vec<serde_json::Value> =
        names.iter().map(|name| json!({ "name": name })).collect();
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: json!({ "prompts": prompts_json }),
    }
}

#[tokio::test]
async fn discovery_cache_fetches_on_first_access() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_tools_response(&["tool_a", "tool_b"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    let tools = cache.tools().await.unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "tool_a");
    assert_eq!(tools[1].name, "tool_b");

    // One request should have been sent
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
}

#[tokio::test]
async fn discovery_cache_returns_cached_on_second_access() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    // Enqueue two responses, but only the first should be consumed
    MockTransport::enqueue_response(&state, make_tools_response(&["tool_a"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    // First access
    let tools1 = cache.tools().await.unwrap();
    assert_eq!(tools1.len(), 1);

    // Second access — should use cache, no new request
    let tools2 = cache.tools().await.unwrap();
    assert_eq!(tools2.len(), 1);
    assert_eq!(tools2[0].name, "tool_a");

    // Only one request should have been sent
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
}

#[tokio::test]
async fn discovery_cache_fetches_resources() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_resources_response(&["file:///a", "file:///b"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    let resources = cache.resources().await.unwrap();
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].uri, "file:///a");
}

#[tokio::test]
async fn discovery_cache_fetches_prompts() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_prompts_response(&["greet", "summarize"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    let prompts = cache.prompts().await.unwrap();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].name, "greet");
}

#[tokio::test]
async fn invalidate_tools_forces_refetch() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    // First response: 1 tool
    MockTransport::enqueue_response(&state, make_tools_response(&["tool_a"]));
    // Second response (after invalidation): 2 tools
    MockTransport::enqueue_response(&state, make_tools_response(&["tool_a", "tool_b"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    // First access
    let tools1 = cache.tools().await.unwrap();
    assert_eq!(tools1.len(), 1);

    // Invalidate and re-fetch
    cache.invalidate_tools().await;
    let tools2 = cache.tools().await.unwrap();
    assert_eq!(tools2.len(), 2);

    // Two requests should have been sent
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 2);
    // Second request should be tools/list
    assert_eq!(s.requests[1].method, "tools/list");
}

#[tokio::test]
async fn invalidate_all_clears_all_caches() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    // Enqueue initial responses for tools, resources, prompts
    MockTransport::enqueue_response(&state, make_tools_response(&["t1"]));
    MockTransport::enqueue_response(&state, make_resources_response(&["r1"]));
    MockTransport::enqueue_response(&state, make_prompts_response(&["p1"]));
    // Enqueue second-round responses after invalidation
    MockTransport::enqueue_response(&state, make_tools_response(&["t1", "t2"]));
    MockTransport::enqueue_response(&state, make_resources_response(&["r1", "r2"]));
    MockTransport::enqueue_response(&state, make_prompts_response(&["p1", "p2"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    // First access — all three
    let tools = cache.tools().await.unwrap();
    let resources = cache.resources().await.unwrap();
    let prompts = cache.prompts().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(resources.len(), 1);
    assert_eq!(prompts.len(), 1);

    // Invalidate all
    cache.invalidate_all().await;

    // Re-fetch — should get updated counts
    let tools = cache.tools().await.unwrap();
    let resources = cache.resources().await.unwrap();
    let prompts = cache.prompts().await.unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(resources.len(), 2);
    assert_eq!(prompts.len(), 2);

    // Should have sent 6 requests total (3 initial + 3 after invalidation)
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 6);
}

#[tokio::test]
async fn invalidate_resources_forces_refetch() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_resources_response(&["r1"]));
    MockTransport::enqueue_response(&state, make_resources_response(&["r1", "r2"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    let r1 = cache.resources().await.unwrap();
    assert_eq!(r1.len(), 1);

    cache.invalidate_resources().await;
    let r2 = cache.resources().await.unwrap();
    assert_eq!(r2.len(), 2);
}

#[tokio::test]
async fn invalidate_prompts_forces_refetch() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_prompts_response(&["p1"]));
    MockTransport::enqueue_response(&state, make_prompts_response(&["p1", "p2", "p3"]));

    let client = Arc::new(McpClient::new(
        "test",
        Box::new(MockTransport::new(state.clone())),
    ));
    let cache = DiscoveryCache::new(client);

    let p1 = cache.prompts().await.unwrap();
    assert_eq!(p1.len(), 1);

    cache.invalidate_prompts().await;
    let p2 = cache.prompts().await.unwrap();
    assert_eq!(p2.len(), 3);
}
