use super::PluginOverlay;
use agent_core::facade::{
    PluginCatalogEntry, PluginComponentInventoryView, PluginInstallTarget,
    PluginMarketplaceSourceView, PluginSettingsView,
};
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode};

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, PluginOverlaySnapshot,
};

fn installed_plugin(settings_id: &str, enabled: bool) -> PluginSettingsView {
    PluginSettingsView {
        settings_id: settings_id.to_string(),
        id: settings_id.replace(':', "-"),
        name: settings_id.to_string(),
        description: format!("{settings_id} plugin"),
        version: Some("1.2.3".to_string()),
        scope: ConfigScope::User,
        path: format!("/tmp/{settings_id}"),
        enabled,
        install_source: Some("local".to_string()),
        marketplace: Some("local-market".to_string()),
        effective: true,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        inventory: PluginComponentInventoryView {
            skill_count: 2,
            skill_names: vec!["alpha".to_string(), "beta".to_string()],
            mcp_server_count: 1,
            app_count: 0,
            agent_count: 1,
            hook_count: 0,
        },
        manifest_kind: "kairox".to_string(),
    }
}

fn catalog_entry_from(marketplace_id: &str, name: &str) -> PluginCatalogEntry {
    PluginCatalogEntry {
        marketplace_id: marketplace_id.to_string(),
        name: name.to_string(),
        description: format!("{name} catalog plugin"),
        version: Some("0.1.0".to_string()),
        source: format!("/tmp/catalog/{name}"),
    }
}

fn catalog_entry(name: &str) -> PluginCatalogEntry {
    catalog_entry_from("local-market", name)
}

fn source(id: &str, enabled: bool) -> PluginMarketplaceSourceView {
    PluginMarketplaceSourceView {
        id: id.to_string(),
        display_name: id.to_string(),
        source: format!("/tmp/{id}"),
        enabled,
        builtin: false,
    }
}

fn snapshot() -> PluginOverlaySnapshot {
    PluginOverlaySnapshot {
        plugins: vec![
            installed_plugin("user:alpha", true),
            installed_plugin("user:beta", false),
        ],
        catalog: vec![
            catalog_entry("delta"),
            catalog_entry_from("remote-market", "epsilon"),
        ],
        sources: vec![source("local-market", true), source("remote-market", true)],
        install_target: PluginInstallTarget::User,
    }
}

fn test_ctx() -> EventContext<'static> {
    use agent_core::projection::SessionProjection;
    static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
    let projection = PROJECTION.get_or_init(SessionProjection::default);
    static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
        std::sync::OnceLock::new();
    let sessions = SESSIONS.get_or_init(Vec::new);
    EventContext {
        focus: FocusTarget::PluginOverlay,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: Box::leak(Box::new(agent_core::WorkspaceId::new())),
        current_session_id: Box::leak(Box::new(None)),
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ))
}

fn type_text(overlay: &mut PluginOverlay, text: &str) {
    for ch in text.chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
}

#[test]
fn lists_installed_plugins_from_snapshot() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());

    assert!(overlay.is_visible());
    assert_eq!(overlay.selected_index(), Some(0));
    assert_eq!(overlay.plugins().len(), 2);

    let backend = ratatui::backend::TestBackend::new(120, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");
    let buf = terminal.backend().buffer().clone();
    let mut rendered = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            rendered.push_str(buf[(x, y)].symbol());
        }
        rendered.push('\n');
    }
    assert!(
        rendered.contains("user:alpha"),
        "installed plugin missing: {rendered}"
    );
    assert!(
        rendered.contains("enabled"),
        "enabled marker missing: {rendered}"
    );
}

#[test]
fn e_toggles_selected_installed_plugin_enabled_state() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));

    assert!(matches!(
        &commands[..],
        [Command::SetPluginEnabled { settings_id, enabled }]
            if settings_id == "user:alpha" && !enabled
    ));
}

#[test]
fn x_deletes_selected_installed_plugin() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));

    assert!(matches!(
        &commands[..],
        [Command::DeletePluginSettings { settings_id }] if settings_id == "user:alpha"
    ));
}

#[test]
fn i_installs_selected_catalog_plugin_to_current_target() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));

    assert!(matches!(
        &commands[..],
        [Command::InstallPlugin { request }]
            if request.marketplace_id == "local-market"
                && request.plugin_name == "delta"
                && request.target == PluginInstallTarget::User
    ));
}

#[test]
fn t_changes_catalog_install_target() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));

    assert!(matches!(
        &commands[..],
        [Command::InstallPlugin { request }]
            if request.plugin_name == "delta" && request.target == PluginInstallTarget::Project
    ));
}

#[test]
fn slash_search_updates_catalog_keyword_and_requests_refresh() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('/')));
    assert!(commands.is_empty());
    type_text(&mut overlay, "delta");
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));
    let filters = overlay.catalog_filters();
    assert_eq!(filters.keyword.as_deref(), Some("delta"));
    assert_eq!(filters.marketplace_id, None);
}

#[test]
fn s_cycles_catalog_marketplace_filter_and_requests_refresh() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('s')));

    assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));
    let filters = overlay.catalog_filters();
    assert_eq!(filters.marketplace_id.as_deref(), Some("local-market"));
    assert_eq!(filters.keyword, None);
}

#[test]
fn e_toggles_selected_marketplace_source() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));

    assert!(matches!(
        &commands[..],
        [Command::SetPluginMarketplaceSourceEnabled { source_id, enabled }]
            if source_id == "local-market" && !enabled
    ));
}

#[test]
fn esc_hides_and_emits_dismiss_effect() {
    let mut overlay = PluginOverlay::new();
    overlay.show(snapshot());

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));

    assert!(commands.is_empty());
    assert!(effects.contains(&CrossPanelEffect::DismissPluginsOverlay));
    assert!(!overlay.is_visible());
}
