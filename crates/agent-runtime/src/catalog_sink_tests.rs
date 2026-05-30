use super::*;
use tokio::sync::broadcast;

#[tokio::test]
async fn sink_forwards_failed_event_to_broadcast() {
    let (tx, mut rx) = broadcast::channel(8);
    let sink = CatalogEventSink::new(tx);
    sink.emit_source_failed("mcp-registry", "timeout").await;
    let ev = rx.recv().await.unwrap();
    match ev.payload {
        EventPayload::CatalogSourceFailed { source, error } => {
            assert_eq!(source, "mcp-registry");
            assert_eq!(error, "timeout");
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[tokio::test]
async fn sink_forwards_added_event_to_broadcast() {
    let (tx, mut rx) = broadcast::channel(8);
    let sink = CatalogEventSink::new(tx);
    sink.emit_source_added("internal").await;
    let ev = rx.recv().await.unwrap();
    assert!(matches!(
        ev.payload,
        EventPayload::CatalogSourceAdded { ref source } if source == "internal"
    ));
}

#[tokio::test]
async fn sink_does_not_panic_when_no_subscribers() {
    let (tx, _) = broadcast::channel::<DomainEvent>(8);
    // Drop receiver immediately by dropping the channel's only rx.
    let sink = CatalogEventSink::new(tx);
    // Should be a silent no-op.
    sink.emit_source_failed("x", "y").await;
    sink.emit_source_added("x").await;
}
