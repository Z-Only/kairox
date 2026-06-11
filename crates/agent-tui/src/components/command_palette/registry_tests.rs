use std::collections::HashSet;

use super::*;

// ── builtin_entries ──────────────────────────────────────────────

#[test]
fn builtin_entries_is_non_empty() {
    assert!(!builtin_entries().is_empty());
}

#[test]
fn builtin_entries_have_unique_ids() {
    let entries = builtin_entries();
    let mut seen = HashSet::new();
    for entry in entries {
        assert!(seen.insert(entry.id.as_ref()), "duplicate id: {}", entry.id);
    }
}

#[test]
fn builtin_entries_have_non_empty_labels() {
    for entry in builtin_entries() {
        assert!(!entry.label.is_empty(), "empty label for id={}", entry.id);
    }
}

#[test]
fn builtin_entries_have_non_empty_descriptions() {
    for entry in builtin_entries() {
        assert!(
            !entry.description.is_empty(),
            "empty description for id={}",
            entry.id
        );
    }
}

// ── filter_entries ───────────────────────────────────────────────

#[test]
fn filter_empty_returns_all() {
    let entries = builtin_entries();
    let filtered = filter_entries("", entries);
    assert_eq!(filtered.len(), entries.len());
}

#[test]
fn filter_whitespace_only_returns_all() {
    let entries = builtin_entries();
    let filtered = filter_entries("   ", entries);
    assert_eq!(filtered.len(), entries.len());
}

#[test]
fn filter_specific_keyword_matches_label() {
    let entries = builtin_entries();
    let filtered = filter_entries(":clear", entries);
    assert!(
        filtered.iter().any(|e| e.id == "clear"),
        "expected :clear to match 'clear' entry"
    );
}

#[test]
fn filter_by_description_word() {
    let entries = builtin_entries();
    // "compaction" appears in the compact entry description
    let filtered = filter_entries("compaction", entries);
    assert!(
        filtered.iter().any(|e| e.id == "compact"),
        "expected 'compaction' to match the compact entry"
    );
}

#[test]
fn filter_by_id_substring() {
    let entries = builtin_entries();
    let filtered = filter_entries("mcp-manager", entries);
    assert!(
        filtered.iter().any(|e| e.id == "mcp-manager"),
        "expected filter by id to match"
    );
}

#[test]
fn filter_no_match_returns_empty() {
    let entries = builtin_entries();
    let filtered = filter_entries("zzz_no_match_zzz", entries);
    assert!(filtered.is_empty());
}

#[test]
fn filter_is_case_insensitive() {
    let entries = builtin_entries();
    let lower = filter_entries("mcp", entries);
    let upper = filter_entries("MCP", entries);
    let mixed = filter_entries("McP", entries);
    assert_eq!(lower.len(), upper.len());
    assert_eq!(lower.len(), mixed.len());
    assert!(!lower.is_empty());
}

#[test]
fn filter_multi_token_matches_all_tokens() {
    let entries = builtin_entries();
    // "open" + "manager" should match entries containing both words
    let filtered = filter_entries("open manager", entries);
    assert!(!filtered.is_empty());
    for entry in &filtered {
        let hay = format!(
            "{} {} {}",
            entry.label.to_lowercase(),
            entry.description.to_lowercase(),
            entry.id.to_lowercase()
        );
        assert!(
            hay.contains("open") && hay.contains("manager"),
            "entry {} does not contain both tokens",
            entry.id
        );
    }
}

#[test]
fn filter_against_custom_entries() {
    let custom = vec![
        PaletteEntry::dynamic("a", "Alpha command", "first entry", PaletteAction::Clear),
        PaletteEntry::dynamic("b", "Beta command", "second entry", PaletteAction::Compact),
    ];
    let filtered = filter_entries("alpha", &custom);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "a");
}

// ── prefill_text ─────────────────────────────────────────────────

#[test]
fn prefill_text_returns_some_for_prefill_model() {
    assert_eq!(prefill_text(&PaletteAction::PrefillModel), Some(":model "));
}

#[test]
fn prefill_text_returns_some_for_goal() {
    assert_eq!(prefill_text(&PaletteAction::PrefillGoal), Some(":goal "));
}

#[test]
fn prefill_text_returns_some_for_prefill_attach() {
    assert_eq!(
        prefill_text(&PaletteAction::PrefillAttach),
        Some(":attach ")
    );
}

#[test]
fn prefill_text_returns_some_for_prefill_detach() {
    assert_eq!(
        prefill_text(&PaletteAction::PrefillDetach),
        Some(":detach ")
    );
}

#[test]
fn prefill_text_returns_some_for_prefill_detach_all() {
    assert_eq!(
        prefill_text(&PaletteAction::PrefillDetachAll),
        Some(":detach")
    );
}

#[test]
fn prefill_text_returns_some_for_prefill_skill_variants() {
    let cases = [
        (PaletteAction::PrefillSkillShow, ":skill show "),
        (PaletteAction::PrefillSkillActivate, ":skill activate "),
        (PaletteAction::PrefillSkillDeactivate, ":skill deactivate "),
        (PaletteAction::PrefillSkillCatalog, ":skill catalog "),
        (PaletteAction::PrefillSkillInstall, ":skill install "),
        (
            PaletteAction::PrefillSkillInstallGithub,
            ":skill install github ",
        ),
        (PaletteAction::PrefillSkillUpdate, ":skill update "),
        (PaletteAction::PrefillSkillDelete, ":skill delete "),
    ];
    for (action, expected) in &cases {
        assert_eq!(
            prefill_text(action),
            Some(*expected),
            "mismatch for {action:?}"
        );
    }
}

#[test]
fn prefill_text_returns_some_for_project_prefills() {
    assert_eq!(
        prefill_text(&PaletteAction::PrefillProjectCreate),
        Some(":project create ")
    );
    assert_eq!(
        prefill_text(&PaletteAction::PrefillProjectImport),
        Some(":project import ")
    );
    assert_eq!(
        prefill_text(&PaletteAction::PrefillProjectWorktree),
        Some(":project worktree ")
    );
}

#[test]
fn prefill_text_returns_some_for_monitor_stop() {
    assert_eq!(
        prefill_text(&PaletteAction::PrefillMonitorStop),
        Some(":monitor stop ")
    );
}

#[test]
fn prefill_text_returns_none_for_zero_arg_actions() {
    let zero_arg = [
        PaletteAction::Clear,
        PaletteAction::Compact,
        PaletteAction::CancelSession,
        PaletteAction::NewSession,
        PaletteAction::ProjectDraftSession,
        PaletteAction::ConfigDir,
        PaletteAction::McpManager,
        PaletteAction::McpConfig,
        PaletteAction::Hooks,
        PaletteAction::Instructions,
        PaletteAction::Plugins,
        PaletteAction::Agents,
        PaletteAction::AgentsDir,
        PaletteAction::Skills,
        PaletteAction::SkillsDir,
        PaletteAction::SkillsManager,
        PaletteAction::SystemPrompt,
        PaletteAction::ModelSelector,
        PaletteAction::ProfilesConfig,
        PaletteAction::SettingsSourceUser,
        PaletteAction::SettingsSourceProject,
        PaletteAction::SettingsProjectNext,
        PaletteAction::SettingsProjectPrevious,
        PaletteAction::RefreshSkillCatalog,
        PaletteAction::MonitorManager,
        PaletteAction::MonitorList,
        PaletteAction::ExportTrace,
        PaletteAction::RefreshConfig,
    ];
    for action in &zero_arg {
        assert_eq!(prefill_text(action), None, "expected None for {action:?}");
    }
}

#[test]
fn prefill_text_returns_none_for_queue_action() {
    let action = PaletteAction::QueueAction(QueueAction::SendSelectedNow);
    assert_eq!(prefill_text(&action), None);
}

#[test]
fn prefill_text_returns_none_for_switch_model() {
    let action = PaletteAction::SwitchModel {
        alias: "gpt-4".into(),
    };
    assert_eq!(prefill_text(&action), None);
}

#[test]
fn prefill_text_returns_none_for_activate_skill() {
    let action = PaletteAction::ActivateSkill {
        skill_id: "some-skill".into(),
    };
    assert_eq!(prefill_text(&action), None);
}

// ── PaletteAction enum coverage ──────────────────────────────────

#[test]
fn palette_action_debug_format() {
    // Smoke test that Debug is derived and doesn't panic.
    let _ = format!("{:?}", PaletteAction::Clear);
    let _ = format!("{:?}", PaletteAction::SwitchModel { alias: "x".into() });
    let _ = format!(
        "{:?}",
        PaletteAction::ActivateSkill {
            skill_id: "y".into()
        }
    );
    let _ = format!(
        "{:?}",
        PaletteAction::QueueAction(QueueAction::DeleteSelected)
    );
}

#[test]
fn palette_action_clone_eq() {
    let a = PaletteAction::PrefillModel;
    let b = a.clone();
    assert_eq!(a, b);

    let c = PaletteAction::SwitchModel {
        alias: "test".into(),
    };
    let d = c.clone();
    assert_eq!(c, d);
}

#[test]
fn palette_action_ne_for_different_variants() {
    assert_ne!(PaletteAction::Clear, PaletteAction::Compact);
    assert_ne!(
        PaletteAction::SwitchModel { alias: "a".into() },
        PaletteAction::SwitchModel { alias: "b".into() }
    );
}

// ── PaletteEntry equality ────────────────────────────────────────

#[test]
fn palette_entry_eq_by_all_fields() {
    let a = PaletteEntry::dynamic("id1", "Label", "Desc", PaletteAction::Clear);
    let b = PaletteEntry::dynamic("id1", "Label", "Desc", PaletteAction::Clear);
    assert_eq!(a, b);
}

#[test]
fn palette_entry_ne_different_id() {
    let a = PaletteEntry::dynamic("id1", "Label", "Desc", PaletteAction::Clear);
    let b = PaletteEntry::dynamic("id2", "Label", "Desc", PaletteAction::Clear);
    assert_ne!(a, b);
}

#[test]
fn palette_entry_ne_different_action() {
    let a = PaletteEntry::dynamic("id1", "Label", "Desc", PaletteAction::Clear);
    let b = PaletteEntry::dynamic("id1", "Label", "Desc", PaletteAction::Compact);
    assert_ne!(a, b);
}

// ── builtin prefill consistency ──────────────────────────────────

#[test]
fn every_prefill_entry_has_prefill_text() {
    // Every builtin entry whose label contains `<` (arg placeholder) should
    // have a non-None prefill_text for its action.
    for entry in builtin_entries() {
        if entry.label.contains('<') {
            assert!(
                prefill_text(&entry.action).is_some(),
                "entry {} has arg placeholder in label but prefill_text returns None",
                entry.id
            );
        }
    }
}

#[test]
fn prefill_text_values_end_with_space_for_arg_commands() {
    // Prefill text for argument-taking commands (except :detach without space)
    // should end with a space so the user can type the argument directly.
    for entry in builtin_entries() {
        if let Some(text) = prefill_text(&entry.action) {
            // :detach (PrefillDetachAll) is the only exception — no trailing space
            if entry.action == PaletteAction::PrefillDetachAll {
                assert!(
                    !text.ends_with(' '),
                    "PrefillDetachAll should not end with space"
                );
            } else {
                assert!(
                    text.ends_with(' '),
                    "prefill for {} should end with space, got {:?}",
                    entry.id,
                    text
                );
            }
        }
    }
}
