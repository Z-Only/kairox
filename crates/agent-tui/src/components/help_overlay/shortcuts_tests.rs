use super::*;

// ---------------------------------------------------------------------------
// is_overlay_focus
// ---------------------------------------------------------------------------

#[test]
fn is_overlay_focus_false_for_non_overlay_targets() {
    assert!(!is_overlay_focus(FocusTarget::Chat));
    assert!(!is_overlay_focus(FocusTarget::Sessions));
    assert!(!is_overlay_focus(FocusTarget::Trace));
}

#[test]
fn is_overlay_focus_true_for_overlay_targets() {
    let overlay_targets = [
        FocusTarget::PermissionModal,
        FocusTarget::McpOverlay,
        FocusTarget::CommandPalette,
        FocusTarget::SkillsOverlay,
        FocusTarget::ModelOverlay,
        FocusTarget::AgentOverlay,
        FocusTarget::PluginOverlay,
        FocusTarget::MonitorOverlay,
        FocusTarget::HooksOverlay,
        FocusTarget::InstructionsOverlay,
    ];
    for target in overlay_targets {
        assert!(
            is_overlay_focus(target),
            "{target:?} should be an overlay focus"
        );
    }
}

// ---------------------------------------------------------------------------
// current_label_prefix
// ---------------------------------------------------------------------------

#[test]
fn current_label_prefix_returns_focus_for_non_overlays() {
    assert_eq!(current_label_prefix(FocusTarget::Chat), "Current focus: ");
    assert_eq!(
        current_label_prefix(FocusTarget::Sessions),
        "Current focus: "
    );
    assert_eq!(current_label_prefix(FocusTarget::Trace), "Current focus: ");
}

#[test]
fn current_label_prefix_returns_overlay_for_overlays() {
    assert_eq!(
        current_label_prefix(FocusTarget::McpOverlay),
        "Current overlay: "
    );
    assert_eq!(
        current_label_prefix(FocusTarget::CommandPalette),
        "Current overlay: "
    );
    assert_eq!(
        current_label_prefix(FocusTarget::PermissionModal),
        "Current overlay: "
    );
}

// ---------------------------------------------------------------------------
// current_label
// ---------------------------------------------------------------------------

#[test]
fn current_label_returns_correct_strings() {
    assert_eq!(current_label(FocusTarget::Chat), "Chat composer");
    assert_eq!(current_label(FocusTarget::Sessions), "Sessions panel");
    assert_eq!(current_label(FocusTarget::Trace), "Trace panel");
    assert_eq!(
        current_label(FocusTarget::PermissionModal),
        "Permission prompt"
    );
    assert_eq!(current_label(FocusTarget::McpOverlay), "MCP manager");
    assert_eq!(
        current_label(FocusTarget::CommandPalette),
        "Command palette"
    );
    assert_eq!(current_label(FocusTarget::SkillsOverlay), "Skills manager");
    assert_eq!(current_label(FocusTarget::ModelOverlay), "Model selector");
    assert_eq!(current_label(FocusTarget::AgentOverlay), "Agent settings");
    assert_eq!(current_label(FocusTarget::PluginOverlay), "Plugin manager");
    assert_eq!(
        current_label(FocusTarget::MonitorOverlay),
        "Monitor manager"
    );
    assert_eq!(current_label(FocusTarget::HooksOverlay), "Hooks settings");
    assert_eq!(
        current_label(FocusTarget::InstructionsOverlay),
        "Instructions settings"
    );
}

// ---------------------------------------------------------------------------
// global_shortcuts
// ---------------------------------------------------------------------------

#[test]
fn global_shortcuts_is_non_empty() {
    assert!(!global_shortcuts().is_empty());
}

#[test]
fn global_shortcuts_contains_f1() {
    assert!(
        global_shortcuts().iter().any(|s| s.key == "F1"),
        "global shortcuts should contain F1"
    );
}

// ---------------------------------------------------------------------------
// common_commands
// ---------------------------------------------------------------------------

#[test]
fn common_commands_is_non_empty() {
    assert!(!common_commands().is_empty());
}

#[test]
fn common_commands_contains_compact() {
    assert!(
        common_commands().iter().any(|s| s.key == ":compact"),
        "common commands should contain :compact"
    );
}

// ---------------------------------------------------------------------------
// context_shortcuts
// ---------------------------------------------------------------------------

#[test]
fn context_shortcuts_non_empty_for_all_variants() {
    let all_targets = [
        FocusTarget::Chat,
        FocusTarget::Sessions,
        FocusTarget::Trace,
        FocusTarget::PermissionModal,
        FocusTarget::McpOverlay,
        FocusTarget::CommandPalette,
        FocusTarget::SkillsOverlay,
        FocusTarget::ModelOverlay,
        FocusTarget::AgentOverlay,
        FocusTarget::PluginOverlay,
        FocusTarget::MonitorOverlay,
        FocusTarget::HooksOverlay,
        FocusTarget::InstructionsOverlay,
    ];
    for target in all_targets {
        assert!(
            !context_shortcuts(target).is_empty(),
            "context_shortcuts({target:?}) should be non-empty"
        );
    }
}

#[test]
fn context_shortcuts_chat_contains_enter() {
    assert!(
        context_shortcuts(FocusTarget::Chat)
            .iter()
            .any(|s| s.key == "Enter"),
        "Chat context shortcuts should contain Enter"
    );
}

#[test]
fn context_shortcuts_sessions_contains_f2() {
    assert!(
        context_shortcuts(FocusTarget::Sessions)
            .iter()
            .any(|s| s.key == "F2"),
        "Sessions context shortcuts should contain F2"
    );
}
