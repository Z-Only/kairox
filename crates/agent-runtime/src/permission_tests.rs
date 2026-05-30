use super::*;

fn pending_map() -> PendingPermissionsMap {
    Arc::new(Mutex::new(HashMap::new()))
}

fn approve(request_id: &str) -> PermissionDecision {
    PermissionDecision {
        request_id: request_id.to_string(),
        approve: true,
        reason: None,
    }
}

fn deny(request_id: &str, reason: &str) -> PermissionDecision {
    PermissionDecision {
        request_id: request_id.to_string(),
        approve: false,
        reason: Some(reason.to_string()),
    }
}

#[tokio::test]
async fn resolve_permission_delivers_approval_to_waiting_receiver() {
    let pending = pending_map();
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending.lock().await.insert("call-1".to_string(), tx);

    resolve_permission(&pending, "call-1", approve("call-1"))
        .await
        .expect("resolve should succeed");

    let decision = rx.await.expect("sender should be alive");
    assert!(decision.approve);
    assert_eq!(decision.request_id, "call-1");
    assert!(decision.reason.is_none());

    // The pending entry is consumed by remove() and not reinserted.
    assert!(pending.lock().await.is_empty());
}

#[tokio::test]
async fn resolve_permission_delivers_denial_with_reason() {
    let pending = pending_map();
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending.lock().await.insert("call-2".to_string(), tx);

    resolve_permission(&pending, "call-2", deny("call-2", "blocked by policy"))
        .await
        .expect("resolve should succeed");

    let decision = rx.await.expect("sender should be alive");
    assert!(!decision.approve);
    assert_eq!(decision.reason.as_deref(), Some("blocked by policy"));
}

#[tokio::test]
async fn resolve_permission_is_a_noop_when_request_id_is_unknown() {
    let pending = pending_map();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    pending.lock().await.insert("call-3".to_string(), tx);

    resolve_permission(&pending, "call-unknown", approve("call-unknown"))
        .await
        .expect("resolve should succeed even when request_id is missing");

    // The unrelated entry is left intact — only matching ids are removed.
    let map = pending.lock().await;
    assert!(map.contains_key("call-3"));
    assert_eq!(map.len(), 1);
}

#[tokio::test]
async fn resolve_permission_drops_decision_silently_when_receiver_already_gone() {
    let pending = pending_map();
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending.lock().await.insert("call-4".to_string(), tx);
    // Drop the receiver to simulate a UI that abandoned the request.
    drop(rx);

    // The tx.send call inside resolve_permission will return Err but we
    // explicitly discard it; the function should still return Ok and
    // remove the entry.
    resolve_permission(&pending, "call-4", approve("call-4"))
        .await
        .expect("resolve should not surface a closed-receiver error");

    assert!(pending.lock().await.is_empty());
}

#[tokio::test]
async fn resolve_permission_only_removes_the_targeted_entry() {
    let pending = pending_map();
    let (tx_a, rx_a) = tokio::sync::oneshot::channel();
    let (tx_b, _rx_b) = tokio::sync::oneshot::channel();
    {
        let mut map = pending.lock().await;
        map.insert("a".to_string(), tx_a);
        map.insert("b".to_string(), tx_b);
    }

    resolve_permission(&pending, "a", approve("a"))
        .await
        .expect("resolve a");

    // Receiver `a` got the decision; entry `b` is still pending.
    let decision = rx_a.await.expect("a sender alive");
    assert!(decision.approve);

    let map = pending.lock().await;
    assert!(!map.contains_key("a"));
    assert!(map.contains_key("b"));
    assert_eq!(map.len(), 1);
}
