use super::*;
use agent_core::ContextSource;

fn make_section(source: ContextSource, tokens: u64) -> (ContextSource, String, u64) {
    (source, format!("{source:?} content"), tokens)
}

#[test]
fn drops_image_first() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::Image, 200),
        make_section(ContextSource::History, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::Image);
}

#[test]
fn drops_selected_file_before_tool_result() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::SelectedFile, 200),
        make_section(ContextSource::ToolResult, 150),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::SelectedFile);
}

#[test]
fn drops_tool_result_before_history() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::ToolResult, 200),
        make_section(ContextSource::History, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::ToolResult);
}

#[test]
fn drops_history_before_memory() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::History, 200),
        make_section(ContextSource::Memory, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::History);
}

#[test]
fn drops_workspace_retrieval_after_history_before_memory() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::WorkspaceRetrieval, 120),
        make_section(ContextSource::Memory, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::WorkspaceRetrieval);

    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::History, 120),
        make_section(ContextSource::WorkspaceRetrieval, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::History);
}

#[test]
fn never_drops_system_or_request() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
    ];
    assert!(find_lowest_priority_drop(&sections).is_none());
}

#[test]
fn drops_project_instruction_before_tool_definitions() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::ProjectInstruction, 300),
        make_section(ContextSource::ToolDefinitions, 500),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::ProjectInstruction);
}

#[test]
fn drops_tool_definitions_before_skill() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::Request, 50),
        make_section(ContextSource::ToolDefinitions, 500),
        make_section(ContextSource::Skill, 200),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    assert_eq!(sections[idx].0, ContextSource::ToolDefinitions);
}

#[test]
fn empty_sections_returns_none() {
    let sections: Vec<(ContextSource, String, u64)> = Vec::new();
    assert!(find_lowest_priority_drop(&sections).is_none());
}

#[test]
fn drops_first_occurrence_of_lowest_priority() {
    let sections = vec![
        make_section(ContextSource::System, 100),
        make_section(ContextSource::History, 50),
        make_section(ContextSource::History, 80),
    ];
    let idx = find_lowest_priority_drop(&sections).unwrap();
    // Should find the first History entry
    assert_eq!(idx, 1);
    assert_eq!(sections[idx].2, 50);
}
