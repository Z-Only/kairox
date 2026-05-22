use super::*;
use agent_core::facade::{
    SkillCatalogEntry, SkillInstallSource, SkillInstallTarget, SkillSettingsScope,
    SkillSettingsView, SkillSourceView, SkillUpdateState,
};
use crossterm::event::{Event, KeyCode};

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, SessionInfo, SkillEntry,
    SkillOverlaySnapshot,
};

fn entry(id: &str, active: bool) -> SkillEntry {
    SkillEntry {
        id: id.to_string(),
        name: id.to_string(),
        description: format!("{id} description"),
        source: "user".to_string(),
        activation_mode: "manual".to_string(),
        active,
    }
}

fn installed_skill(skill_id: &str, enabled: bool) -> SkillSettingsView {
    SkillSettingsView {
        settings_id: format!("user:{skill_id}"),
        id: skill_id.to_string(),
        name: skill_id.to_string(),
        description: format!("{skill_id} settings"),
        version: Some("1.0.0".to_string()),
        scope: SkillSettingsScope::User,
        path: format!("/tmp/{skill_id}/SKILL.md"),
        enabled,
        activation_mode: "manual".to_string(),
        install_source: SkillInstallSource::Registry,
        update_state: SkillUpdateState::UpdateAvailable,
        effective: enabled,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: true,
        deletable: true,
    }
}

fn catalog_entry(name: &str) -> SkillCatalogEntry {
    SkillCatalogEntry {
        catalog_id: "skillhub".to_string(),
        name: name.to_string(),
        description: format!("{name} catalog skill"),
        source: "skillhub".to_string(),
        source_url: format!("https://example.test/{name}"),
        install_count: Some(42),
        github_stars: Some(7),
        security_score: Some(95),
        rating: Some(4.8),
        package: name.to_string(),
        package_url: Some(format!("https://example.test/{name}.zip")),
    }
}

fn source(id: &str, enabled: bool) -> SkillSourceView {
    SkillSourceView {
        id: id.to_string(),
        display_name: id.to_string(),
        kind: "skillhub".to_string(),
        url: format!("https://example.test/{id}"),
        search_template: "/api/skills?q={{query}}".to_string(),
        download_template: "/api/download/{{slug}}".to_string(),
        list_template: Some("/api/skills".to_string()),
        detail_template: None,
        field_mapping: agent_core::facade::SkillFieldMappingView::default(),
        enabled,
        priority: 10,
        cache_ttl_seconds: 900,
        last_error: None,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ))
}

fn type_text(overlay: &mut SkillsOverlay, text: &str) {
    for ch in text.chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
}

fn render_overlay_text(overlay: &SkillsOverlay) -> String {
    let backend = ratatui::backend::TestBackend::new(140, 32);
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
    rendered
}

fn test_ctx_session(
    session_id: &Option<agent_core::SessionId>,
    workspace_id: &agent_core::WorkspaceId,
) -> EventContext<'static> {
    use agent_core::projection::SessionProjection;
    static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
    let projection = PROJECTION.get_or_init(SessionProjection::default);
    static SESSIONS: std::sync::OnceLock<Vec<SessionInfo>> = std::sync::OnceLock::new();
    let sessions = SESSIONS.get_or_init(Vec::new);
    // The component only reads `workspace_id` and `current_session_id` —
    // leak owned copies so the static-lifetime EventContext compiles for
    // tests without us having to thread a runtime through.
    let ws: &'static agent_core::WorkspaceId = Box::leak(Box::new(workspace_id.clone()));
    let sid: &'static Option<agent_core::SessionId> = Box::leak(Box::new(session_id.clone()));
    EventContext {
        focus: FocusTarget::SkillsOverlay,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        permission_mode: agent_tools::PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: ws,
        current_session_id: sid,
    }
}

fn test_ctx() -> EventContext<'static> {
    let ws = agent_core::WorkspaceId::new();
    let sid: Option<agent_core::SessionId> = Some(agent_core::SessionId::new());
    test_ctx_session(&sid, &ws)
}

#[test]
fn lists_skills_with_active_marker() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", true), entry("beta", false)]);
    assert!(overlay.is_visible());
    assert_eq!(overlay.skills().len(), 2);
    assert_eq!(overlay.selected_index(), Some(0));

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
    assert!(rendered.contains("alpha"), "alpha row missing: {rendered}");
    assert!(rendered.contains("beta"), "beta row missing: {rendered}");
    assert!(
        rendered.contains("active"),
        "active marker missing for active skill: {rendered}"
    );
}

#[test]
fn overlay_invisible_by_default() {
    let overlay = SkillsOverlay::new();
    assert!(!overlay.is_visible());
    assert!(overlay.skills().is_empty());
}

#[test]
fn j_and_k_navigate_selection() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![
        entry("alpha", false),
        entry("beta", true),
        entry("gamma", false),
    ]);
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(2));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
    assert_eq!(overlay.selected_index(), Some(2));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(1));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
    assert_eq!(overlay.selected_index(), Some(0));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(0));
}

#[test]
fn enter_emits_show_skill_for_selected() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false), entry("beta", false)]);
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(
        &commands[0],
        Command::ShowSkill { skill_id } if skill_id == "beta"
    ));
}

#[test]
fn body_effect_switches_to_detail_view() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false)]);
    overlay.handle_effect(&CrossPanelEffect::ShowSkillBody {
        skill_id: "alpha".to_string(),
        body: "## Body\n\nDoc text".to_string(),
    });
    assert_eq!(overlay.body_skill_id(), Some("alpha"));

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
    assert!(rendered.contains("Doc text"), "body text missing");

    // Esc in body view returns to the list, not dismiss.
    let (effects, _) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
    assert!(effects.is_empty());
    assert!(overlay.is_visible());
    assert_eq!(overlay.body_skill_id(), None);
}

#[test]
fn a_emits_activate_for_inactive_skill() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false)]);
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
    assert!(matches!(
        &commands[0],
        Command::ActivateSkill { skill_id, .. } if skill_id == "alpha"
    ));
}

#[test]
fn a_is_no_op_for_already_active_skill() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", true)]);
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
    assert!(commands.is_empty());
}

#[test]
fn d_emits_deactivate_for_active_skill() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", true)]);
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('d')));
    assert!(matches!(
        &commands[0],
        Command::DeactivateSkill { skill_id, .. } if skill_id == "alpha"
    ));
}

#[test]
fn a_without_session_emits_nothing() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false)]);
    let ws = agent_core::WorkspaceId::new();
    let ctx = test_ctx_session(&None, &ws);
    let (_, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('a')));
    assert!(commands.is_empty());
}

#[test]
fn esc_hides_and_emits_dismiss_effect() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false)]);
    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
    assert!(commands.is_empty());
    assert!(effects.contains(&CrossPanelEffect::DismissSkillsOverlay));
    assert!(!overlay.is_visible());
}

#[test]
fn ignores_keys_when_hidden() {
    let mut overlay = SkillsOverlay::new();
    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
}

#[test]
fn show_effect_makes_visible() {
    let mut overlay = SkillsOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowSkillsOverlay(
        vec![entry("alpha", false)].into(),
    ));
    assert!(overlay.is_visible());
    assert_eq!(overlay.skills().len(), 1);
}

#[test]
fn show_preserves_selection_across_refresh() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(vec![entry("alpha", false), entry("beta", false)]);
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));
    // Same list, beta now active — selection should stay on beta.
    overlay.show(vec![entry("alpha", false), entry("beta", true)]);
    assert_eq!(overlay.selected_index(), Some(1));
    assert!(overlay.skills()[1].active);
}

#[test]
fn installed_tab_dispatches_enable_update_and_delete_commands() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![entry("alpha", false)],
        installed: vec![installed_skill("review", true)],
        catalog: vec![],
        sources: vec![],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
    assert!(matches!(
        &enable_commands[..],
        [Command::SetSkillEnabled { skill_id, enabled }]
            if skill_id == "review" && !enabled
    ));

    let (_, update_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('u')));
    assert!(matches!(
        &update_commands[..],
        [Command::UpdateSkillSettings { skill_id }] if skill_id == "review"
    ));

    let (_, delete_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
    assert!(matches!(
        &delete_commands[..],
        [Command::DeleteSkillSettings { skill_id }] if skill_id == "review"
    ));
}

#[test]
fn catalog_tab_installs_selected_entry_to_current_target() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![catalog_entry("review")],
        sources: vec![],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(matches!(
        &install_commands[..],
        [Command::InstallRemoteSkill { request }]
            if request.package == "review"
                && request.source == "skillhub"
                && request.target == SkillInstallTarget::User
                && request.package_url.as_deref() == Some("https://example.test/review.zip")
    ));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    let (_, project_install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(matches!(
        &project_install_commands[..],
        [Command::InstallRemoteSkill { request }]
            if request.package == "review" && request.target == SkillInstallTarget::Project
    ));
}

#[test]
fn catalog_enter_opens_detail_and_esc_returns_to_list() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![catalog_entry("review")],
        sources: vec![],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
    let rendered = render_overlay_text(&overlay);
    assert!(
        rendered.contains("Source: https://example.test/review"),
        "source URL missing from detail: {rendered}"
    );
    assert!(
        rendered.contains("Package: review"),
        "package missing from detail: {rendered}"
    );
    assert!(
        rendered.contains("Download: https://example.test/review.zip"),
        "download URL missing from detail: {rendered}"
    );
    assert!(
        rendered.contains("Installs: 42"),
        "install stats missing from detail: {rendered}"
    );
    assert!(
        rendered.contains("Target: user"),
        "target confirmation missing from detail: {rendered}"
    );

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
    assert!(overlay.is_visible());
    let rendered = render_overlay_text(&overlay);
    assert!(
        rendered.contains("review catalog skill"),
        "Esc should return to catalog list: {rendered}"
    );
}

#[test]
fn catalog_detail_installs_selected_entry_to_current_target() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![catalog_entry("review")],
        sources: vec![],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    let rendered = render_overlay_text(&overlay);
    assert!(
        rendered.contains("Target: project"),
        "target toggle should update detail confirmation: {rendered}"
    );

    let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(matches!(
        &install_commands[..],
        [Command::InstallRemoteSkill { request }]
            if request.package == "review"
                && request.source == "skillhub"
                && request.target == SkillInstallTarget::Project
                && request.package_url.as_deref() == Some("https://example.test/review.zip")
    ));
}

#[test]
fn catalog_tab_searches_and_refreshes_with_active_source_filter() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![catalog_entry("review")],
        sources: vec![source("skillhub", true), source("corp", true)],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, source_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
    assert!(matches!(
        &source_commands[..],
        [Command::ListSkillCatalog { keyword: None, sources: Some(sources) }]
            if sources == &vec!["skillhub".to_string()]
    ));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('/')));
    type_text(&mut overlay, "review");
    let (_, search_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(
        &search_commands[..],
        [Command::ListSkillCatalog { keyword: Some(keyword), sources: Some(sources) }]
            if keyword == "review" && sources == &vec!["skillhub".to_string()]
    ));

    let (_, refresh_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
    assert!(matches!(
        &refresh_commands[..],
        [Command::RefreshSkillCatalog { keyword: Some(keyword), sources: Some(sources) }]
            if keyword == "review" && sources == &vec!["skillhub".to_string()]
    ));
}

#[test]
fn sources_tab_toggles_selected_source() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![],
        sources: vec![source("skillhub", true)],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
    assert!(matches!(
        &commands[..],
        [Command::SetSkillSourceEnabled { source_id, enabled }]
            if source_id == "skillhub" && !enabled
    ));
}

#[test]
fn sources_tab_adds_and_removes_skill_sources() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![],
        installed: vec![],
        catalog: vec![],
        sources: vec![source("skillhub", true)],
        install_target: SkillInstallTarget::User,
    });
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

    let (_, remove_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
    assert!(matches!(
        &remove_commands[..],
        [Command::RemoveSkillSource { source_id }] if source_id == "skillhub"
    ));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
    assert!(commands.is_empty());
    type_text(&mut overlay, "corp");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "Corporate Skills");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "https://skills.example.com");
    let (_, add_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(
        &add_commands[..],
        [Command::AddSkillSource { config }]
            if config.id == "corp"
                && config.display_name == "Corporate Skills"
                && config.kind == "skillhub"
                && config.url == "https://skills.example.com"
                && config.search_template == "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc"
                && config.download_template == "/api/v1/download?slug={{slug}}"
                && config.list_template.as_deref() == Some("/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc")
                && config.detail_template.as_deref() == Some("/api/v1/skills/{{slug}}")
                && config.enabled
                && config.priority == 100
                && config.cache_ttl_seconds == 900
                && config.last_error.is_none()
    ));
}

#[test]
fn discovered_tab_keeps_session_activation_commands() {
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: vec![entry("alpha", false)],
        installed: vec![installed_skill("alpha", true)],
        catalog: vec![catalog_entry("alpha")],
        sources: vec![source("skillhub", true)],
        install_target: SkillInstallTarget::User,
    });

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));

    assert!(matches!(
        &commands[..],
        [Command::ActivateSkill { skill_id, .. }] if skill_id == "alpha"
    ));
}
