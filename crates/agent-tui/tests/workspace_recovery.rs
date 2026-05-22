use std::path::PathBuf;

use agent_tui::workspace_recovery::{
    format_known_workspaces, parse_workspace_args, resolve_workspace_selector, KnownWorkspace,
    WorkspaceCliMode,
};

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "kairox-tui-workspace-recovery-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp project");
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn known(id: &str, path: &str) -> KnownWorkspace {
    KnownWorkspace {
        workspace_id: id.to_string(),
        path: path.to_string(),
    }
}

#[test]
fn workspace_recovery_parses_list_select_and_selector_flags() {
    let list = parse_workspace_args(["--workspace-list"]).expect("list flag should parse");
    assert_eq!(list.mode, WorkspaceCliMode::List);

    let select = parse_workspace_args(["--workspace-select"]).expect("select flag should parse");
    assert_eq!(select.mode, WorkspaceCliMode::Select);

    let selector =
        parse_workspace_args(["--workspace", "wrk_project"]).expect("selector should parse");
    assert_eq!(
        selector.mode,
        WorkspaceCliMode::Use("wrk_project".to_string())
    );
}

#[test]
fn workspace_recovery_formats_known_workspaces_for_terminal_listing() {
    let output = format_known_workspaces(&[
        known("wrk_a", "/tmp/project-a"),
        known("wrk_b", "/tmp/project-b"),
    ]);

    assert!(output.contains("Known workspaces"));
    assert!(output.contains("1. wrk_a  /tmp/project-a"));
    assert!(output.contains("2. wrk_b  /tmp/project-b"));
}

#[test]
fn workspace_recovery_resolves_known_workspace_by_index_id_or_path() {
    let project_a = TempProject::new("project-a");
    let project_b = TempProject::new("project-b");
    let workspaces = [
        known("wrk_a", &project_a.path().display().to_string()),
        known("wrk_b", &project_b.path().display().to_string()),
    ];

    assert_eq!(
        resolve_workspace_selector(&workspaces, "2").expect("index selector"),
        project_b.path
    );
    assert_eq!(
        resolve_workspace_selector(&workspaces, "wrk_a").expect("id selector"),
        project_a.path
    );
    assert_eq!(
        resolve_workspace_selector(&workspaces, &project_b.path().display().to_string())
            .expect("path selector"),
        project_b.path
    );
}

#[test]
fn workspace_recovery_rejects_stale_known_workspace_paths() {
    let workspaces = [known("wrk_missing", "/tmp/kairox-missing-workspace")];

    let error = resolve_workspace_selector(&workspaces, "wrk_missing")
        .expect_err("missing known path should be rejected");

    assert!(error.contains("workspace path does not exist"));
    assert!(error.contains("wrk_missing"));
}

#[test]
fn workspace_recovery_accepts_existing_direct_path_selector() {
    let temp = TempProject::new("direct");
    let selected = resolve_workspace_selector(&[], &temp.path().display().to_string())
        .expect("direct path selector");

    assert_eq!(selected, PathBuf::from(temp.path().display().to_string()));
}
