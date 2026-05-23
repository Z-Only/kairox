use agent_core::ContextSource;

/// Find the index of the lowest-priority section that can be dropped.
///
/// Priority (highest first): System, Skill, ToolDefinitions, Request, Memory,
/// History, ToolResult, SelectedFile.
///
/// `System` and `Request` are never dropped; `Skill` is dropped only after
/// `ToolDefinitions`.
pub(super) fn find_lowest_priority_drop(
    sections: &[(ContextSource, String, u64)],
) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
        ContextSource::ProjectInstruction,
        ContextSource::ToolDefinitions,
        ContextSource::Skill,
    ];
    for category in &drop_order {
        for (i, (src, _, _)) in sections.iter().enumerate() {
            if src == category {
                return Some(i);
            }
        }
    }
    None
}
