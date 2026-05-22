use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownWorkspace {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCliMode {
    CurrentDir,
    List,
    Select,
    Use(String),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCli {
    pub mode: WorkspaceCliMode,
}

pub fn parse_workspace_args<I, S>(args: I) -> Result<WorkspaceCli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(first) = args.next() else {
        return Ok(WorkspaceCli {
            mode: WorkspaceCliMode::CurrentDir,
        });
    };

    let mode = match first.as_str() {
        "--help" | "-h" => WorkspaceCliMode::Help,
        "--workspace-list" | "--workspaces" => WorkspaceCliMode::List,
        "--workspace-select" => WorkspaceCliMode::Select,
        "--workspace" | "-w" => {
            let Some(selector) = args.next() else {
                return Err(format!(
                    "--workspace requires an id, index, or path\n{}",
                    workspace_usage()
                ));
            };
            WorkspaceCliMode::Use(selector)
        }
        other => {
            return Err(format!("unknown argument: {other}\n{}", workspace_usage()));
        }
    };

    if let Some(extra) = args.next() {
        return Err(format!(
            "unexpected argument: {extra}\n{}",
            workspace_usage()
        ));
    }

    Ok(WorkspaceCli { mode })
}

pub fn workspace_usage() -> &'static str {
    "Usage: kairox-tui [--workspace-list | --workspace-select | --workspace <id|index|path>]\n\n\
Workspace options:\n  \
--workspace-list              List known workspaces and exit\n  \
--workspace-select            Prompt for a known workspace before launch\n  \
--workspace, -w <id|index|path>  Launch in a known workspace or existing path"
}

pub fn format_known_workspaces(workspaces: &[KnownWorkspace]) -> String {
    if workspaces.is_empty() {
        return "No known workspaces.\n".to_string();
    }

    let mut output = String::from("Known workspaces:\n");
    for (index, workspace) in workspaces.iter().enumerate() {
        output.push_str(&format!(
            "{}. {}  {}\n",
            index + 1,
            workspace.workspace_id,
            workspace.path
        ));
    }
    output
}

pub fn prompt_workspace_selector(workspaces: &[KnownWorkspace]) -> Result<Option<String>, String> {
    print!("{}", format_known_workspaces(workspaces));
    print!("Select workspace number, id, or path (blank to cancel): ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush prompt: {error}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("failed to read workspace selection: {error}"))?;
    let selector = input.trim();
    if selector.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selector.to_string()))
    }
}

pub fn resolve_workspace_selector(
    workspaces: &[KnownWorkspace],
    selector: &str,
) -> Result<PathBuf, String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err("workspace selector cannot be empty".to_string());
    }

    if let Ok(index) = selector.parse::<usize>() {
        if let Some(workspace) = index
            .checked_sub(1)
            .and_then(|zero_based| workspaces.get(zero_based))
        {
            return existing_known_workspace_path(workspace);
        }
    }

    if let Some(workspace) = workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == selector || workspace.path == selector)
    {
        return existing_known_workspace_path(workspace);
    }

    let direct = PathBuf::from(selector);
    if direct.is_dir() {
        return Ok(direct);
    }

    Err(format!(
        "workspace selector not found: {selector}. Run --workspace-list to see known workspaces."
    ))
}

fn existing_known_workspace_path(workspace: &KnownWorkspace) -> Result<PathBuf, String> {
    let path = PathBuf::from(&workspace.path);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!(
            "workspace path does not exist for {}: {}",
            workspace.workspace_id, workspace.path
        ))
    }
}
