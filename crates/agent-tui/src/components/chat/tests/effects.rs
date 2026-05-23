//! Cross-panel `handle_effect` paths that are not permission-specific —
//! today this covers the streaming start/stop no-op handling.

use super::super::*;

#[test]
fn handle_effect_start_stop_streaming_noop() {
    let mut panel = ChatPanel::new();
    panel.handle_effect(&CrossPanelEffect::StartStreaming);
    panel.handle_effect(&CrossPanelEffect::StopStreaming);
    // Just verifying no panic and state unchanged.
    assert_eq!(panel.input_state, InputState::Normal);
}
