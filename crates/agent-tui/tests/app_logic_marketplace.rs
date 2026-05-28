//! TUI App logic integration tests — plugin marketplace overlay refresh.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use app_logic_common::TuiMcpFakeFacade;
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn tui_plugin_overlay_refresh_passes_catalog_filters_to_facade() {
    use agent_core::facade::{PluginInstallTarget, PluginMarketplaceSourceView};
    use agent_core::projection::SessionProjection;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{
        Command, Component, EventContext, FocusTarget, PluginOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let workspace_id = WorkspaceId::new();
    let current_session_id = None;
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::PluginOverlay,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: &workspace_id,
        current_session_id: &current_session_id,
    };
    let mut app = App::new("fake", workspace_id.clone());
    app.plugin_overlay.show(PluginOverlaySnapshot {
        plugins: Vec::new(),
        catalog: Vec::new(),
        sources: vec![PluginMarketplaceSourceView {
            id: "local-market".into(),
            display_name: "Local market".into(),
            source: "/tmp/local-market".into(),
            enabled: true,
            builtin: false,
        }],
        install_target: PluginInstallTarget::User,
    });

    let _ = app.plugin_overlay.handle_event(&ctx, &key(KeyCode::Tab));
    let _ = app
        .plugin_overlay
        .handle_event(&ctx, &key(KeyCode::Char('s')));
    let _ = app
        .plugin_overlay
        .handle_event(&ctx, &key(KeyCode::Char('/')));
    for ch in "delta".chars() {
        let _ = app
            .plugin_overlay
            .handle_event(&ctx, &key(KeyCode::Char(ch)));
    }
    let (_, commands) = app.plugin_overlay.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));

    agent_tui::app::dispatch_commands(&runtime, &mut app, commands).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_plugin_catalog:Some(\"local-market\"):Some(\"delta\")"),
        "expected filtered plugin catalog call, got {calls:?}"
    );
}
