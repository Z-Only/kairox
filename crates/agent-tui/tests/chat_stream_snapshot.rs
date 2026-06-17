//! Snapshot tests for the unified chat-stream renderer.
//!
//! Locks down the per-scenario rendering of
//! [`agent_tui::components::chat::render_chat_stream`] so future drift
//! in the inline chat-stream renderer is caught by CI. Mirrors the
//! discrete `ChatStreamItem` variants emitted by
//! `agent_tui::components::chat::stream::fold_stream`.
//!
//! These assertions deliberately key off *glyphs and labels* (e.g.
//! `▸`, `▾`, `⟳`, `✓`, `✗`, `╭`, `│`, `╰`, the literal status words
//! `requested` / `running` / `completed` / `failed`, etc.) rather than
//! whole-buffer pixel diffs — a pixel diff would be too brittle while
//! still missing the semantic shifts we actually care about (a glyph
//! moving rows, a status label silently disappearing, a permission
//! border collapsing). Companion suite to
//! `tests/chat_render.rs`, which exercises the same renderer with
//! broader behavioural checks.

use std::collections::HashSet;

use agent_core::events::{CompactionReason, CompactionSkipReason, EventPayload, MonitorStopReason};
use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use agent_tui::app_state::InputState;
use agent_tui::components::chat::render_chat_stream;
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Build a domain event anchored at `offset_ms` past the Unix epoch so
/// scenarios can interleave events with deterministic chronological
/// ordering without needing real wall-clock time.
fn make_event_at(offset_ms: i64, payload: EventPayload) -> DomainEvent {
    let timestamp = Utc.timestamp_opt(0, 0).unwrap() + ChronoDuration::milliseconds(offset_ms);
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
    .with_timestamp(timestamp)
}

/// Render `projection` + `events` into a `TestBackend` of the requested
/// dimensions and return the resulting buffer flattened to a string
/// (one row per line, trailing whitespace preserved by ratatui).
fn render_to_string(
    width: u16,
    height: u16,
    projection: &SessionProjection,
    events: &[DomainEvent],
    expanded: &HashSet<String>,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            render_chat_stream(
                frame.area(),
                frame,
                projection,
                events,
                expanded,
                &InputState::Normal,
            );
        })
        .expect("render_chat_stream should not panic");
    terminal.backend().to_string()
}

/// Construct a `SessionProjection` populated only with the given
/// chat messages — every other field defaults to a quiescent value so
/// the renderer's compaction/cancellation/token-stream branches stay
/// dormant.
fn projection_with_messages(messages: Vec<(ProjectedRole, &str)>) -> SessionProjection {
    SessionProjection {
        messages: messages
            .into_iter()
            .map(|(role, content)| ProjectedMessage {
                role,
                content: content.to_string(),
            })
            .collect(),
        task_titles: vec![],
        task_graph: agent_core::facade::TaskGraphSnapshot::default(),
        token_stream: String::new(),
        cancelled: false,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    }
}

/// Glyphs the renderer is contractually required to use for non-message
/// rows. Tests assert presence/absence of these to detect silent drift.
const TOOL_CALL_COLLAPSED_GLYPH: char = '▸';
const TOOL_CALL_EXPANDED_GLYPH: char = '▾';
const COMPACTION_RUNNING_GLYPH: char = '⟳';
const COMPACTION_COMPLETED_GLYPH: char = '✓';
const COMPACTION_FAILED_GLYPH: char = '✗';
const PERMISSION_BORDER_TOP: char = '╭';
const PERMISSION_BORDER_LEFT: char = '│';
const PERMISSION_BORDER_BOTTOM: char = '╰';

// ---------------------------------------------------------------------------
// 1. Empty stream → just renders header/empty state.
// ---------------------------------------------------------------------------

#[test]
fn empty_stream_renders_no_chrome_or_glyphs() {
    let projection = projection_with_messages(vec![]);
    let events: Vec<DomainEvent> = vec![];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    // Without any messages or events, the renderer should produce a
    // blank buffer — no role labels, no permission box, no tool-call
    // marker, no compaction banner.
    for needle in [
        "You:",
        "Agent:",
        "Compacting",
        "Compacted",
        "Compaction failed",
        "Permission required",
        "Memory write required",
    ] {
        assert!(
            !output.contains(needle),
            "empty stream should not render `{needle}`; got:\n{output}"
        );
    }
    for glyph in [
        TOOL_CALL_COLLAPSED_GLYPH,
        TOOL_CALL_EXPANDED_GLYPH,
        COMPACTION_RUNNING_GLYPH,
        COMPACTION_COMPLETED_GLYPH,
        COMPACTION_FAILED_GLYPH,
        PERMISSION_BORDER_TOP,
        PERMISSION_BORDER_LEFT,
        PERMISSION_BORDER_BOTTOM,
    ] {
        assert!(
            !output.contains(glyph),
            "empty stream should not render glyph `{glyph}`; got:\n{output}"
        );
    }

    // The rendered cells themselves should all be blank — strip ratatui's
    // `to_string` row framing (`"…"\n`) before checking so the assertion
    // doesn't trip on the formatter's own characters.
    let body: String = output
        .lines()
        .map(|line| line.trim_matches('"'))
        .collect::<Vec<_>>()
        .join("");
    assert!(
        body.chars().all(|c| c == ' '),
        "empty stream output should be entirely blank cells; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 2. Single user message from projection.messages only.
// ---------------------------------------------------------------------------

#[test]
fn single_user_message_renders_role_label_only() {
    let projection =
        projection_with_messages(vec![(ProjectedRole::User, "hello from snapshot tests")]);
    let events: Vec<DomainEvent> = vec![];
    let expanded = HashSet::new();

    let output = render_to_string(80, 12, &projection, &events, &expanded);

    assert!(
        output.contains("You:"),
        "user message should render `You:` label; got:\n{output}"
    );
    assert!(
        output.contains("hello from snapshot tests"),
        "user message content should appear; got:\n{output}"
    );

    // No other affordances should appear for a bare message.
    assert!(
        !output.contains("Agent:"),
        "no assistant label expected; got:\n{output}"
    );
    for glyph in [
        TOOL_CALL_COLLAPSED_GLYPH,
        TOOL_CALL_EXPANDED_GLYPH,
        COMPACTION_RUNNING_GLYPH,
        COMPACTION_COMPLETED_GLYPH,
        COMPACTION_FAILED_GLYPH,
        PERMISSION_BORDER_TOP,
    ] {
        assert!(
            !output.contains(glyph),
            "single user message should not render glyph `{glyph}`; got:\n{output}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. User + assistant messages mixed with one ToolCall (collapsed).
// ---------------------------------------------------------------------------

#[test]
fn collapsed_tool_call_renders_marker_and_status_after_messages() {
    let projection = projection_with_messages(vec![
        (ProjectedRole::User, "please list files"),
        (ProjectedRole::Assistant, "running shell.exec for you"),
    ]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            150,
            EventPayload::ToolInvocationStarted {
                invocation_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            260,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "call_1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "do-not-leak-this-line".into(),
                exit_code: Some(0),
                duration_ms: 110,
                truncated: false,
                images: vec![],
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 16, &projection, &events, &expanded);

    let pos_user = output.find("You:").expect("user role label should render");
    let pos_assistant = output
        .find("Agent:")
        .expect("assistant role label should render");
    let pos_tool = output
        .find("▸ shell.exec")
        .expect("collapsed tool-call marker should render");

    assert!(
        pos_user < pos_assistant,
        "messages should render in projection order (user before assistant); \
         got user={pos_user} assistant={pos_assistant}\n{output}"
    );
    assert!(
        pos_assistant < pos_tool,
        "tool-call row should render below the messages; \
         got assistant={pos_assistant} tool={pos_tool}\n{output}"
    );

    assert!(
        output.contains("completed"),
        "collapsed tool call should carry its terminal status label; got:\n{output}"
    );
    assert!(
        !output.contains(TOOL_CALL_EXPANDED_GLYPH),
        "collapsed tool call must not use the expanded glyph `{TOOL_CALL_EXPANDED_GLYPH}`; \
         got:\n{output}"
    );
    assert!(
        !output.contains("do-not-leak-this-line"),
        "collapsed (non-failed) tool call must not leak its output preview; got:\n{output}"
    );
    assert!(
        !output.contains("exit=0"),
        "collapsed tool call must not surface exit code; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 4. ToolCall expanded.
// ---------------------------------------------------------------------------

#[test]
fn expanded_tool_call_renders_output_and_exit_code() {
    let projection = projection_with_messages(vec![(ProjectedRole::User, "tail the log")]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_42".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            220,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "call_42".into(),
                tool_id: "shell.exec".into(),
                output_preview: "line-one\nline-two".into(),
                exit_code: Some(0),
                duration_ms: 250,
                truncated: false,
                images: vec![],
            },
        ),
    ];

    let mut expanded = HashSet::new();
    expanded.insert("call_42".to_string());

    let output = render_to_string(80, 16, &projection, &events, &expanded);

    assert!(
        output.contains(TOOL_CALL_EXPANDED_GLYPH),
        "expanded tool call should render the expanded marker `{TOOL_CALL_EXPANDED_GLYPH}`; \
         got:\n{output}"
    );
    assert!(
        !output.contains(TOOL_CALL_COLLAPSED_GLYPH),
        "expanded tool call should NOT also render the collapsed marker; got:\n{output}"
    );

    assert!(
        output.contains("output:"),
        "expanded tool call should label its output block; got:\n{output}"
    );
    assert!(
        output.contains("line-one"),
        "expanded tool call should reveal first preview line; got:\n{output}"
    );
    assert!(
        output.contains("line-two"),
        "expanded tool call should reveal subsequent preview lines; got:\n{output}"
    );
    assert!(
        output.contains("exit=0"),
        "expanded tool call should display its exit code; got:\n{output}"
    );
    assert!(
        output.contains("completed"),
        "expanded tool call header should still carry the status label; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 5. PermissionItem alone → boxed prompt with tool_id + preview.
// ---------------------------------------------------------------------------

#[test]
fn pending_permission_alone_renders_full_boxed_prompt() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        50,
        EventPayload::PermissionRequested {
            request_id: "req_1".into(),
            tool_id: "shell.exec".into(),
            preview: "rm -rf /tmp/foo".into(),
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(80, 12, &projection, &events, &expanded);

    // Box chrome.
    assert!(
        output.contains(PERMISSION_BORDER_TOP),
        "permission row should render the top border glyph `{PERMISSION_BORDER_TOP}`; \
         got:\n{output}"
    );
    assert!(
        output.contains(PERMISSION_BORDER_LEFT),
        "permission row should render left border glyphs `{PERMISSION_BORDER_LEFT}`; \
         got:\n{output}"
    );
    assert!(
        output.contains(PERMISSION_BORDER_BOTTOM),
        "permission row should render bottom border glyph `{PERMISSION_BORDER_BOTTOM}`; \
         got:\n{output}"
    );

    // Header copy + recovered tool id + preview.
    assert!(
        output.contains("Permission required"),
        "permission header should announce itself; got:\n{output}"
    );
    assert!(
        output.contains("tool:"),
        "permission row should label the tool id; got:\n{output}"
    );
    assert!(
        output.contains("shell.exec"),
        "permission row should include the recovered tool id; got:\n{output}"
    );
    assert!(
        output.contains("preview:"),
        "permission row should label the preview block; got:\n{output}"
    );
    assert!(
        output.contains("rm -rf /tmp/foo"),
        "permission row should show the preview text; got:\n{output}"
    );
    assert!(
        output.contains("Y/N/D"),
        "permission row should hint at the allow / deny key bindings; got:\n{output}"
    );

    // It must not look like the tool-id was lost; the renderer falls
    // back to `(unknown)` if the pre-pass map fails to recover the id.
    assert!(
        !output.contains("(unknown)"),
        "permission row should not fall back to `(unknown)` when the tool_id is \
         recoverable from the PermissionRequested event; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 6. CompactionItem Running → progress banner.
// ---------------------------------------------------------------------------

#[test]
fn running_compaction_renders_progress_banner() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        500,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::UserRequested,
            before_tokens: 120_000,
            candidate_event_count: 24,
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(80, 8, &projection, &events, &expanded);

    assert!(
        output.contains(COMPACTION_RUNNING_GLYPH),
        "running compaction should render the `{COMPACTION_RUNNING_GLYPH}` progress glyph; \
         got:\n{output}"
    );
    assert!(
        output.contains("Compacting context..."),
        "running compaction should render the progress banner copy; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_COMPLETED_GLYPH),
        "running compaction should not also render the completed glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "running compaction should not also render the failed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 7. CompactionItem Completed → check glyph.
// ---------------------------------------------------------------------------

#[test]
fn completed_compaction_renders_check_glyph_only() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            500,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            900,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_complete".into(),
                after_tokens: 28_000,
                fallback_used: false,
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 8, &projection, &events, &expanded);

    assert!(
        output.contains(COMPACTION_COMPLETED_GLYPH),
        "completed compaction should render the `{COMPACTION_COMPLETED_GLYPH}` glyph; \
         got:\n{output}"
    );
    assert!(
        output.contains("Compacted"),
        "completed compaction should render the `Compacted` copy; got:\n{output}"
    );
    assert!(
        !output.contains("Compacting"),
        "completed compaction must not still announce `Compacting`; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "completed compaction must not also render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "completed compaction must not also render the failed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 7b. CompactionItem Completed with both before/after token counts →
//     prepends a "{before} → {after} tokens (-{pct}%)" segment to the
//     standard `✓ Compacted` row. Failed and missing-token cases keep
//     their previous behaviour.
// ---------------------------------------------------------------------------

#[test]
fn compaction_completed_with_token_savings() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            500,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 25_000,
                candidate_event_count: 12,
            },
        ),
        make_event_at(
            900,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_savings".into(),
                after_tokens: 12_000,
                fallback_used: false,
            },
        ),
    ];
    let expanded = HashSet::new();

    // Wider buffer so the prepended token segment + the standard
    // `✓ Compacted` label fit on a single visible row.
    let output = render_to_string(120, 8, &projection, &events, &expanded);

    assert!(
        output.contains(COMPACTION_COMPLETED_GLYPH),
        "completed compaction with token savings should still render the \
         `{COMPACTION_COMPLETED_GLYPH}` glyph; got:\n{output}"
    );
    assert!(
        output.contains("Compacted"),
        "completed compaction with token savings should still render `Compacted`; \
         got:\n{output}"
    );
    assert!(
        output.contains("25000 → 12000"),
        "completed compaction should prepend the raw before→after token counts; \
         got:\n{output}"
    );
    assert!(
        output.contains("tokens"),
        "completed compaction should label the delta as `tokens`; got:\n{output}"
    );
    assert!(
        output.contains("-52"),
        "completed compaction should surface the percentage savings (-52%); \
         got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "completed compaction must not still render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "completed compaction must not also render the failed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 8. CompactionItem Failed → error styling + error string.
// ---------------------------------------------------------------------------

#[test]
fn failed_compaction_renders_cross_glyph_with_error() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            500,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            900,
            EventPayload::ContextCompactionFailed {
                error: "model timeout".into(),
                fallback_used: false,
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 8, &projection, &events, &expanded);

    assert!(
        output.contains(COMPACTION_FAILED_GLYPH),
        "failed compaction should render the `{COMPACTION_FAILED_GLYPH}` glyph; got:\n{output}"
    );
    assert!(
        output.contains("Compaction failed"),
        "failed compaction should announce failure; got:\n{output}"
    );
    assert!(
        output.contains("model timeout"),
        "failed compaction should surface the error string; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "failed compaction must not also render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_COMPLETED_GLYPH),
        "failed compaction must not also render the completed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 9. CompactionItem Idle → renders nothing.
// ---------------------------------------------------------------------------

#[test]
fn idle_compaction_renders_no_compaction_line() {
    // A normal message-only conversation with no compaction events at
    // all should leave the chat-stream free of compaction chrome.
    let projection = projection_with_messages(vec![
        (ProjectedRole::User, "hello"),
        (ProjectedRole::Assistant, "hi back"),
    ]);
    let events: Vec<DomainEvent> = vec![];
    let expanded = HashSet::new();

    let output = render_to_string(80, 12, &projection, &events, &expanded);

    // The messages should render; the compaction surface should not.
    assert!(
        output.contains("hello"),
        "user message should still render; got:\n{output}"
    );
    assert!(
        output.contains("hi back"),
        "assistant message should still render; got:\n{output}"
    );

    for needle in ["Compact", "Compacting", "Compacted", "Compaction failed"] {
        assert!(
            !output.contains(needle),
            "idle compaction must not render `{needle}`; got:\n{output}"
        );
    }
    for glyph in [
        COMPACTION_RUNNING_GLYPH,
        COMPACTION_COMPLETED_GLYPH,
        COMPACTION_FAILED_GLYPH,
    ] {
        assert!(
            !output.contains(glyph),
            "idle compaction must not render glyph `{glyph}`; got:\n{output}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8b. ContextCompactionSkipped (AlreadyCompacting) → renders a "skipped"
//     line with the human-readable reason phrase and the ratio.
// ---------------------------------------------------------------------------

#[test]
fn skipped_compaction_already_compacting_renders_reason_and_ratio() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        500,
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::AlreadyCompacting,
            ratio: 0.42,
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(120, 8, &projection, &events, &expanded);

    assert!(
        output.contains("Compaction skipped"),
        "skipped compaction should announce itself with `Compaction skipped`; got:\n{output}"
    );
    assert!(
        output.contains("another compaction in flight"),
        "AlreadyCompacting should render the `another compaction in flight` reason phrase; \
         got:\n{output}"
    );
    assert!(
        output.contains("0.42"),
        "skipped compaction should surface the ratio when informative; got:\n{output}"
    );
    // It must not look like an in-flight, completed, or failed compaction.
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "skipped compaction must not render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_COMPLETED_GLYPH),
        "skipped compaction must not render the completed glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "skipped compaction must not render the failed glyph; got:\n{output}"
    );
    assert!(
        !output.contains("Compacting context..."),
        "skipped compaction must not render the running banner; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 8c. ContextCompactionSkipped (ThresholdDisabled) → renders the reason
//     phrase without a misleading "ratio" segment (the threshold is off,
//     so the ratio carries no useful information for the user).
// ---------------------------------------------------------------------------

#[test]
fn skipped_compaction_threshold_disabled_renders_reason_without_ratio() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        500,
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::ThresholdDisabled,
            ratio: 0.91,
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(120, 8, &projection, &events, &expanded);

    assert!(
        output.contains("Compaction skipped"),
        "skipped compaction should announce itself with `Compaction skipped`; got:\n{output}"
    );
    assert!(
        output.contains("threshold disabled"),
        "ThresholdDisabled should render the `threshold disabled` reason phrase; got:\n{output}"
    );
    assert!(
        !output.contains("ratio"),
        "ThresholdDisabled should omit the ratio segment — ratio is moot when threshold is off; \
         got:\n{output}"
    );
    assert!(
        !output.contains("0.91"),
        "ThresholdDisabled should omit the raw ratio value; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "skipped compaction must not render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_COMPLETED_GLYPH),
        "skipped compaction must not render the completed glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "skipped compaction must not render the failed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 8d. ContextCompactionSkipped (NotEnoughHistory) → renders the manual
//     short-history reason without a misleading "ratio" segment.
// ---------------------------------------------------------------------------

#[test]
fn skipped_compaction_not_enough_history_renders_reason_without_ratio() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        500,
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::NotEnoughHistory,
            ratio: 0.0,
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(120, 8, &projection, &events, &expanded);

    assert!(
        output.contains("Compaction skipped"),
        "skipped compaction should announce itself with `Compaction skipped`; got:\n{output}"
    );
    assert!(
        output.contains("not enough history"),
        "NotEnoughHistory should render the `not enough history` reason phrase; got:\n{output}"
    );
    assert!(
        !output.contains("ratio"),
        "NotEnoughHistory should omit the ratio segment; got:\n{output}"
    );
    assert!(
        !output.contains("0.00"),
        "NotEnoughHistory should omit the raw ratio value; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_RUNNING_GLYPH),
        "skipped compaction must not render the running glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_COMPLETED_GLYPH),
        "skipped compaction must not render the completed glyph; got:\n{output}"
    );
    assert!(
        !output.contains(COMPACTION_FAILED_GLYPH),
        "skipped compaction must not render the failed glyph; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 10. Multiple items in chronological order — assert row ordering
//     matches `fold_stream` output (permission, tool, running
//     compaction in that timestamp order, all rendered below the
//     messages).
// ---------------------------------------------------------------------------

#[test]
fn multiple_stream_items_render_in_chronological_order() {
    let projection = projection_with_messages(vec![
        (ProjectedRole::User, "do the dance"),
        (ProjectedRole::Assistant, "ok"),
    ]);
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "do the dance".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "ok".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::PermissionRequested {
                request_id: "req_chrono".into(),
                tool_id: "shell.exec".into(),
                preview: "echo hi".into(),
            },
        ),
        make_event_at(
            40,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_chrono".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            50,
            EventPayload::ToolInvocationStarted {
                invocation_id: "call_chrono".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            60,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 90_000,
                candidate_event_count: 8,
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(120, 24, &projection, &events, &expanded);

    let pos_user = output
        .find("You:")
        .expect("user message should render from projection");
    let pos_assistant = output
        .find("Agent:")
        .expect("assistant message should render from projection");
    let pos_perm_header = output
        .find("Permission required")
        .expect("permission header should render");
    let pos_perm_preview = output
        .find("echo hi")
        .expect("permission preview should render");
    let pos_tool = output
        .find("▸ shell.exec")
        .expect("tool-call marker should render");
    let pos_comp = output
        .find("Compacting context...")
        .expect("compaction banner should render");

    // Messages render first (in projection order), then non-message
    // items in the order `fold_stream` emits them: permission (ts=30),
    // tool call (ts=40), compaction (ts=60).
    assert!(
        pos_user < pos_assistant,
        "user message should render before assistant message; got user={pos_user} \
         assistant={pos_assistant}\n{output}"
    );
    assert!(
        pos_assistant < pos_perm_header,
        "all messages should render before any non-message stream item; got \
         assistant={pos_assistant} permission={pos_perm_header}\n{output}"
    );
    assert!(
        pos_perm_header < pos_perm_preview,
        "permission header must render above its preview; got header={pos_perm_header} \
         preview={pos_perm_preview}\n{output}"
    );
    assert!(
        pos_perm_preview < pos_tool,
        "permission row (ts=30) should render above tool call (ts=40); got \
         permission_preview={pos_perm_preview} tool={pos_tool}\n{output}"
    );
    assert!(
        pos_tool < pos_comp,
        "tool call (ts=40) should render above compaction banner (ts=60); got \
         tool={pos_tool} compaction={pos_comp}\n{output}"
    );

    // Sanity: each of the four non-message item types renders exactly
    // once when no duplicates were emitted.
    assert_eq!(
        output.matches("Permission required").count(),
        1,
        "exactly one permission row should render; got:\n{output}"
    );
    assert_eq!(
        output.matches('▸').count(),
        1,
        "exactly one collapsed tool-call marker should render; got:\n{output}"
    );
    assert_eq!(
        output.matches("Compacting context...").count(),
        1,
        "exactly one compaction banner should render; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 14. Monitor — Running
// ---------------------------------------------------------------------------

#[test]
fn running_monitor_renders_watching_glyph_and_description() {
    let projection = projection_with_messages(vec![]);
    let events = vec![make_event_at(
        100,
        EventPayload::MonitorStarted {
            monitor_id: "mon_snap_1".into(),
            description: "build watcher".into(),
            command: "tail -f build.log".into(),
            persistent: false,
            timeout_ms: 300_000,
        },
    )];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains('⟳'),
        "running monitor should render ⟳ glyph; got:\n{output}"
    );
    assert!(
        output.contains("build watcher"),
        "running monitor should render its description; got:\n{output}"
    );
    assert!(
        output.contains("watching"),
        "running monitor should render 'watching' label; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 15. Monitor — Running with last_line
// ---------------------------------------------------------------------------

#[test]
fn running_monitor_with_event_renders_last_line() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_2".into(),
                description: "log tailer".into(),
                command: "tail -f app.log".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            200,
            EventPayload::MonitorEvent {
                monitor_id: "mon_snap_2".into(),
                line: "ERROR: connection refused".into(),
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains("log tailer"),
        "monitor description should render; got:\n{output}"
    );
    assert!(
        output.contains("ERROR: connection refused"),
        "last_line from MonitorEvent should render; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 16. Monitor — Stopped (exit code 0)
// ---------------------------------------------------------------------------

#[test]
fn stopped_monitor_exit_zero_renders_done_label() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_3".into(),
                description: "build check".into(),
                command: "make test".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            500,
            EventPayload::MonitorStopped {
                monitor_id: "mon_snap_3".into(),
                reason: MonitorStopReason::ExitCode { code: 0 },
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains('■'),
        "stopped monitor should render ■ glyph; got:\n{output}"
    );
    assert!(
        output.contains("done"),
        "exit-0 monitor should render 'done' label; got:\n{output}"
    );
    assert!(
        !output.contains('⟳'),
        "stopped monitor should not render ⟳ (running); got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 17. Monitor — Stopped (non-zero exit)
// ---------------------------------------------------------------------------

#[test]
fn stopped_monitor_nonzero_exit_renders_exited_label() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_4".into(),
                description: "test runner".into(),
                command: "cargo test".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            800,
            EventPayload::MonitorStopped {
                monitor_id: "mon_snap_4".into(),
                reason: MonitorStopReason::ExitCode { code: 1 },
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains("exited"),
        "non-zero exit monitor should render 'exited' label; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 18. Monitor — Stopped (timeout)
// ---------------------------------------------------------------------------

#[test]
fn stopped_monitor_timeout_renders_timed_out_label() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_5".into(),
                description: "long poll".into(),
                command: "watch status".into(),
                persistent: false,
                timeout_ms: 60_000,
            },
        ),
        make_event_at(
            61_000,
            EventPayload::MonitorStopped {
                monitor_id: "mon_snap_5".into(),
                reason: MonitorStopReason::Timeout,
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains("timed out"),
        "timeout monitor should render 'timed out' label; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 19. Monitor — Stopped (user stopped)
// ---------------------------------------------------------------------------

#[test]
fn stopped_monitor_user_stopped_renders_stopped_label() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_6".into(),
                description: "ci watcher".into(),
                command: "gh run watch".into(),
                persistent: true,
                timeout_ms: 0,
            },
        ),
        make_event_at(
            2000,
            EventPayload::MonitorStopped {
                monitor_id: "mon_snap_6".into(),
                reason: MonitorStopReason::UserStopped,
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains("stopped"),
        "user-stopped monitor should render 'stopped' label; got:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// 20. Monitor — Failed
// ---------------------------------------------------------------------------

#[test]
fn failed_monitor_renders_cross_glyph_and_error() {
    let projection = projection_with_messages(vec![]);
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_snap_7".into(),
                description: "broken watcher".into(),
                command: "nonexistent-cmd".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            150,
            EventPayload::MonitorFailed {
                monitor_id: "mon_snap_7".into(),
                error: "No such file or directory".into(),
            },
        ),
    ];
    let expanded = HashSet::new();

    let output = render_to_string(80, 24, &projection, &events, &expanded);

    assert!(
        output.contains('✗'),
        "failed monitor should render ✗ glyph; got:\n{output}"
    );
    assert!(
        output.contains("failed"),
        "failed monitor should render 'failed' label; got:\n{output}"
    );
    assert!(
        output.contains("No such file or directory"),
        "failed monitor should render error as last_line; got:\n{output}"
    );
}
