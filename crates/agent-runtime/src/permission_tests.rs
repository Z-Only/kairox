use super::*;

fn pending_map() -> PendingPermissionsMap {
    Arc::new(Mutex::new(HashMap::new()))
}

fn session(id: &str) -> SessionId {
    SessionId::from_string(id.to_string())
}

async fn insert_pending(
    pending: &PendingPermissionsMap,
    request_id: &str,
    session_id: &SessionId,
    tx: tokio::sync::oneshot::Sender<PermissionDecision>,
) {
    pending.lock().await.insert(
        request_id.to_string(),
        PendingPermission {
            session_id: session_id.clone(),
            reply: tx,
        },
    );
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
    insert_pending(&pending, "call-1", &session("session-1"), tx).await;

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
    insert_pending(&pending, "call-2", &session("session-1"), tx).await;

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
    insert_pending(&pending, "call-3", &session("session-1"), tx).await;

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
    insert_pending(&pending, "call-4", &session("session-1"), tx).await;
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
    let session_id = session("session-1");
    insert_pending(&pending, "a", &session_id, tx_a).await;
    insert_pending(&pending, "b", &session_id, tx_b).await;

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

#[tokio::test]
async fn deny_pending_permissions_for_session_only_denies_matching_session() {
    let pending = pending_map();
    let matching_session = session("matching-session");
    let other_session = session("other-session");
    let (matching_tx, matching_rx) = tokio::sync::oneshot::channel();
    let (other_tx, _other_rx) = tokio::sync::oneshot::channel();
    insert_pending(&pending, "matching-call", &matching_session, matching_tx).await;
    insert_pending(&pending, "other-call", &other_session, other_tx).await;

    let denied = deny_pending_permissions_for_session(&pending, &matching_session, "cancelled")
        .await
        .expect("deny pending");

    assert_eq!(denied, vec!["matching-call".to_string()]);
    let decision = matching_rx.await.expect("matching sender alive");
    assert!(!decision.approve);
    assert_eq!(decision.request_id, "matching-call");
    assert_eq!(decision.reason.as_deref(), Some("cancelled"));

    let map = pending.lock().await;
    assert!(!map.contains_key("matching-call"));
    assert!(map.contains_key("other-call"));
    assert_eq!(map.len(), 1);
}
