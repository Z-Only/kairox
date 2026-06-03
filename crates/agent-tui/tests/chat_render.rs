//! Integration tests for the unified inline chat-stream rendering.
//!
//! Exercises [`agent_tui::components::chat::render_chat_stream`] — the
//! renderer that consumes the [`ChatStreamItem`] folded by
//! `agent_tui::components::chat::stream::fold_stream` and draws messages,
//! permission prompts, tool calls, and compaction status into the chat
//! scrollback. Mirrors the GUI parity established by `ChatMessageItem`,
//! `ChatPermissionItem`, `ChatToolCallItem`, and `ChatCompactionItem`.

use std::collections::HashSet;

use agent_core::events::{CompactionReason, EventPayload};
use agent_core::projection::SessionProjection;
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use agent_tui::components::chat::render_chat_stream;
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

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
            render_chat_stream(frame.area(), frame, projection, events, expanded);
        })
        .expect("render_chat_stream should not panic");
    terminal.backend().to_string()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn renders_messages_then_permission_then_tool_call_then_compaction_running() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "run a shell command".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::PermissionRequested {
                request_id: "req_1".into(),
                tool_id: "shell.exec".into(),
                preview: "ls -la".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            35,
            EventPayload::ToolInvocationStarted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            40,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 120_000,
                candidate_event_count: 12,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 24, &projection, &events, &expanded);

    let pos_user = output
        .find("run a shell command")
        .expect("user message visible");
    let pos_perm = output.find("ls -la").expect("permission preview visible");
    // Tool-call header is marked with the "▸ " collapse marker; the
    // permission row uses a "│ tool:" prefix, so this match is
    // unambiguous.
    let pos_tool = output
        .find("▸ shell.exec")
        .expect("tool call header visible");
    let pos_comp = output
        .find("Compacting")
        .expect("compaction running banner visible");

    assert!(
        pos_user < pos_perm && pos_perm < pos_tool && pos_tool < pos_comp,
        "items should render in chronological order; got positions \
         user={pos_user} perm={pos_perm} tool={pos_tool} comp={pos_comp}\n{output}"
    );
}

#[test]
fn permission_request_render_includes_tool_id_and_preview() {
    let events = vec![make_event_at(
        50,
        EventPayload::PermissionRequested {
            request_id: "req_1".into(),
            tool_id: "shell.exec".into(),
            preview: "rm -rf /tmp/foo".into(),
        },
    )];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("Permission"),
        "permission row should announce itself; got:\n{output}"
    );
    assert!(
        output.contains("shell.exec"),
        "permission row should include tool id; got:\n{output}"
    );
    assert!(
        output.contains("rm -rf /tmp/foo"),
        "permission row should show preview; got:\n{output}"
    );
    assert!(
        output.contains("Y/N/D"),
        "permission row should hint at allow/deny key bindings; got:\n{output}"
    );
}

#[test]
fn tool_call_collapsed_shows_one_line_with_status() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            110,
            EventPayload::ToolInvocationStarted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            220,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "secret-output-line".into(),
                exit_code: Some(0),
                duration_ms: 120,
                truncated: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("shell.exec"),
        "collapsed tool call should show tool id; got:\n{output}"
    );
    assert!(
        output.contains("completed") || output.contains("done"),
        "collapsed tool call should show terminal status; got:\n{output}"
    );
    assert!(
        output.contains("120ms") || output.contains("0.1s"),
        "collapsed tool call should show duration; got:\n{output}"
    );
    assert!(
        !output.contains("secret-output-line"),
        "collapsed tool call must NOT reveal output_preview; got:\n{output}"
    );
}

#[test]
fn tool_call_expanded_shows_output_preview() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            220,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "call_1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "expanded-output-line".into(),
                exit_code: Some(0),
                duration_ms: 120,
                truncated: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    // The tool-call item id is the tool_call_id from ModelToolCallRequested.
    let mut expanded = HashSet::new();
    expanded.insert("call_1".to_string());

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("expanded-output-line"),
        "expanded tool call should reveal output_preview; got:\n{output}"
    );
    assert!(
        output.contains("exit=0") || output.contains("exit 0"),
        "expanded tool call should display exit code; got:\n{output}"
    );
}

#[test]
fn tool_call_failed_shows_error_preview_even_when_collapsed() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            220,
            EventPayload::ToolInvocationFailed {
                invocation_id: "call_1".into(),
                tool_id: "shell.exec".into(),
                error: "boom".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("failed"),
        "failed tool call should render failure status; got:\n{output}"
    );
}

#[test]
fn compaction_idle_renders_nothing_extra() {
    // Only messages — no compaction events — so no compaction line.
    let events = vec![make_event_at(
        10,
        EventPayload::UserMessageAdded {
            message_id: "u1".into(),
            content: "hello".into(),
            display_content: None,
        },
    )];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("hello"),
        "user message should render; got:\n{output}"
    );
    assert!(
        !output.contains("Compact"),
        "no compaction banner should appear when idle; got:\n{output}"
    );
}

#[test]
fn compaction_completed_renders_brief_check() {
    let events = vec![
        make_event_at(
            1_000,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            2_500,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_1".into(),
                after_tokens: 30_000,
                fallback_used: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("Compacted"),
        "completed compaction should render brief check; got:\n{output}"
    );
    assert!(
        !output.contains("Compacting"),
        "completed compaction must not still announce running; got:\n{output}"
    );
}

#[test]
fn compaction_failed_renders_error_styling() {
    let events = vec![
        make_event_at(
            1_000,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            2_500,
            EventPayload::ContextCompactionFailed {
                error: "model timeout".into(),
                fallback_used: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(
        output.contains("Compaction failed") || output.contains("compaction failed"),
        "failed compaction should announce failure; got:\n{output}"
    );
    assert!(
        output.contains("model timeout"),
        "failed compaction should surface the error message; got:\n{output}"
    );
}

#[test]
fn resolved_permission_is_filtered_from_stream() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "go ahead".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::PermissionRequested {
                request_id: "req_1".into(),
                tool_id: "shell.exec".into(),
                preview: "rm -rf /tmp/foo".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::PermissionGranted {
                request_id: "req_1".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    assert!(output.contains("go ahead"), "{output}");
    assert!(
        !output.contains("rm -rf /tmp/foo"),
        "resolved permissions should be filtered from inline stream; got:\n{output}"
    );
}

#[test]
fn only_most_recent_compaction_renders_when_completed() {
    let events = vec![
        make_event_at(
            1_000,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            1_100,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_1".into(),
                after_tokens: 30_000,
                fallback_used: false,
            },
        ),
        make_event_at(
            5_000,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 90_000,
                candidate_event_count: 8,
            },
        ),
        make_event_at(
            6_000,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_2".into(),
                after_tokens: 12_000,
                fallback_used: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let expanded = HashSet::new();

    let output = render_to_string(120, 12, &projection, &events, &expanded);

    let compacted_count = output.matches("Compacted").count();
    assert_eq!(
        compacted_count, 1,
        "only the most recent compaction should render in the stream; \
         got {compacted_count} `Compacted` markers in:\n{output}"
    );
}
