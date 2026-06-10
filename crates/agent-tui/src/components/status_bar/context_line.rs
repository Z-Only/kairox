//! P3 context-meter helpers — token formatting, source labels, and the
//! plain-text status line / details lines rendered when usage is known.

use agent_core::context_types::ContextSource;

use crate::components::StatusInfo;

// ---------------------------------------------------------------------------
// Token formatting helpers
// ---------------------------------------------------------------------------

/// Format a token count as `1.2k` for >=1000, otherwise the raw number.
pub(super) fn fmt_tokens(n: u64) -> String {
    if n >= 1_000 {
        format!("{:.1}k", (n as f64) / 1_000.0)
    } else {
        n.to_string()
    }
}

pub(super) fn percent_of(tokens: u64, budget_tokens: u64) -> u64 {
    if budget_tokens == 0 {
        0
    } else {
        (((tokens as f64) / (budget_tokens as f64)) * 100.0).round() as u64
    }
}

/// Compact form of `fmt_tokens` for per-source breakdown chips: `12k` (no decimal).
pub(super) fn fmt_short(n: u64) -> String {
    if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

pub(super) fn source_label(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::System => "System",
        ContextSource::ToolDefinitions => "Tools",
        ContextSource::Request => "Request",
        ContextSource::Memory => "Memory",
        ContextSource::WorkspaceRetrieval => "Workspace retrieval",
        ContextSource::Git => "Git",
        ContextSource::History => "History",
        ContextSource::ToolResult => "Tool result",
        ContextSource::SelectedFile => "Selected file",
        ContextSource::CompactionSummary => "Compaction summary",
        ContextSource::Skill => "Skill",
        ContextSource::ProjectInstruction => "Project instructions",
        ContextSource::Image => "Image",
    }
}

/// Map a [`ContextSource`] to a 3-5 char chip label for the breakdown line.
pub(super) fn source_short_label(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::System => "sys",
        ContextSource::ToolDefinitions => "tools",
        ContextSource::Request => "req",
        ContextSource::Memory => "mem",
        ContextSource::WorkspaceRetrieval => "rag",
        ContextSource::Git => "git",
        ContextSource::History => "hist",
        ContextSource::ToolResult => "tres",
        ContextSource::SelectedFile => "file",
        ContextSource::CompactionSummary => "csum",
        ContextSource::Skill => "skill",
        ContextSource::ProjectInstruction => "proj",
        ContextSource::Image => "img",
    }
}

// ---------------------------------------------------------------------------
// Public renderers (text-only)
// ---------------------------------------------------------------------------

pub fn render_context_details_lines(info: &StatusInfo) -> Vec<String> {
    let Some(usage) = &info.context_usage else {
        return vec![
            "No context usage yet".to_string(),
            format!(
                "Compaction: {}",
                if info.compacting { "running" } else { "idle" }
            ),
            "[Esc] close".to_string(),
        ];
    };

    let pct = percent_of(usage.total_tokens, usage.budget_tokens);
    let mut lines = vec![
        format!(
            "Used: {} / {} ({}%)",
            fmt_tokens(usage.total_tokens),
            fmt_tokens(usage.budget_tokens),
            pct
        ),
        format!("Context window: {}", fmt_tokens(usage.context_window)),
        format!(
            "Reserved for response: {}",
            fmt_tokens(usage.output_reservation)
        ),
        format!(
            "Compaction: {}",
            if info.compacting { "running" } else { "idle" }
        ),
        "Source breakdown:".to_string(),
    ];

    for (source, tokens) in &usage.by_source {
        lines.push(format!(
            "  {:<20} {:>7} {:>3}%",
            source_label(source),
            fmt_tokens(*tokens),
            percent_of(*tokens, usage.budget_tokens)
        ));
    }

    let compact_hint = if info.compacting {
        "[c] compacting...  [Esc] close"
    } else {
        "[c] compact now  [Esc] close"
    };
    lines.push(compact_hint.to_string());
    lines
}

/// Render a single status line including the context-meter info as a plain
/// `String` (so unit tests can assert on the text without going through
/// ratatui rendering).
///
/// Layout:
/// - Always: `profile: <name>  perm: <mode>`
/// - When `usage.is_some()`:
///   - `width >= 100`: long form `ctx: <tot>/<bud>[ ⚠]  <chip1> <n1> <chip2> <n2> …`
///   - `width <  100`: short form `ctx: <tot>/<bud> (<pct>%)[ ⚠]`
/// - When `usage.is_none()`: `ctx: -`
/// - When `compacting`: appends `compacting…`
pub fn render_context_line_string(info: &StatusInfo, width: u16) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("profile: {}", info.profile));
    let approval = info.approval_policy_label();
    if !approval.is_empty() {
        parts.push(format!("appr: {approval}"));
    }
    let sandbox = info.sandbox_policy_label();
    if !sandbox.is_empty() {
        parts.push(format!("sbox: {sandbox}"));
    }
    if !info.session_metadata.is_empty() {
        parts.push(info.session_metadata.join(" · "));
    }

    match &info.context_usage {
        Some(u) => {
            let pct = if u.budget_tokens == 0 {
                0
            } else {
                (((u.total_tokens as f64) / (u.budget_tokens as f64)) * 100.0).round() as u64
            };
            // Warning glyph at >=70%; the GUI surfaces an additional badge for
            // the >=85% err tier — the TUI keeps a single tier here to stay
            // readable on a one-row status bar.
            let warn = if pct >= 70 { " ⚠" } else { "" };

            if width >= 100 {
                parts.push(format!(
                    "ctx: {}/{}{}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    warn
                ));
                let mut breakdown = String::new();
                for (source, tokens) in &u.by_source {
                    breakdown.push_str(&format!(
                        " {} {}",
                        source_short_label(source),
                        fmt_short(*tokens)
                    ));
                }
                parts.push(breakdown.trim_start().to_string());
            } else {
                parts.push(format!(
                    "ctx: {}/{} ({}%){}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    pct,
                    warn
                ));
            }
        }
        None => parts.push("ctx: -".into()),
    }

    if info.context_usage.is_some() {
        parts.push("Alt+C details".into());
    }

    if info.compacting {
        parts.push("compacting…".into());
    }

    parts.join("  ")
}
