//! Integration test: event payload coverage — every variant has a unique
//! event_type, and a representative subset of variants roundtrip through
//! JSON serde at the EventPayload level (without envelope timestamp variance).

use agent_core::{
    context_types::{ContextSource, ContextUsage},
    AgentRole, CompactionReason, CompactionSkipReason, EventPayload, TaskConfirmationOption,
};
use std::collections::HashSet;

// ── helpers ────────────────────────────────────────────────────────────────

fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
    s.parse()
        .unwrap_or_else(|e| panic!("invalid timestamp '{s}': {e}"))
}

fn usage() -> ContextUsage {
    ContextUsage {
        total_tokens: 1_234,
        budget_tokens: 200_000,
        context_window: 200_000,
        output_reservation: 9_000,
        by_source: vec![
            (ContextSource::System, 800),
            (ContextSource::ToolDefinitions, 434),
        ],
        estimator: "cl100k_base".into(),
        corrected_by_real_usage: false,
    }
}

// ── every variant has a non-empty & unique event_type ──────────────────────

#[test]
fn every_event_payload_variant_has_event_type() {
    // Construct one instance per variant.
    let variants: Vec<EventPayload> = vec![
        EventPayload::WorkspaceOpened {
            path: "/tmp/proj".into(),
        },
        EventPayload::SessionInitialized {
            model_profile: "fast".into(),
        },
        EventPayload::UserMessageAdded {
            message_id: "m1".into(),
            content: "hello".into(),
            display_content: None,
        },
        EventPayload::AgentTaskCreated {
            task_id: agent_core::TaskId::new(),
            title: "inspect".into(),
            role: AgentRole::Planner,
            dependencies: vec![],
        },
        EventPayload::AgentTaskStarted {
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::ContextAssembled { usage: usage() },
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::Threshold { ratio: 0.9 },
            before_tokens: 150_000,
            candidate_event_count: 35,
        },
        EventPayload::ContextCompactionCompleted {
            summary_id: "sum1".into(),
            after_tokens: 12_000,
            fallback_used: false,
        },
        EventPayload::ContextCompactionFailed {
            error: "timeout".into(),
            fallback_used: true,
        },
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::NotEnoughHistory,
            ratio: 0.0,
        },
        EventPayload::CompactionSummary {
            summary_id: "sum1".into(),
            content: "## Summary".into(),
            replaces_event_range: (ts("2026-05-01T00:00:00Z"), ts("2026-05-01T01:00:00Z")),
            reason: CompactionReason::UserRequested,
            before_tokens: 150_000,
            after_tokens: 12_000,
            summarised_by_profile: "fast".into(),
        },
        EventPayload::ModelProfileSwitched {
            from_profile: "fast".into(),
            to_profile: "opus".into(),
            reasoning_effort: Some("high".into()),
            effective_at: ts("2026-05-01T00:00:00Z"),
            context_window: 200_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
        },
        EventPayload::ModelRequestStarted {
            model_profile: "fast".into(),
            model_id: "gpt-4.1-mini".into(),
        },
        EventPayload::ModelUsageRecorded {
            model_profile: "fast".into(),
            input_tokens: 123,
            output_tokens: 45,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(20),
        },
        EventPayload::ModelStreamStatus {
            phase: "stream_start".into(),
            retrying: true,
            retry_attempt: 1,
            max_retries: 1,
            message: "model stream start idle timeout; retrying".into(),
        },
        EventPayload::ModelTokenDelta {
            delta: "Hello".into(),
        },
        EventPayload::ModelToolCallRequested {
            tool_call_id: "call1".into(),
            tool_id: "shell.exec".into(),
        },
        EventPayload::PermissionRequested {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            preview: "rm -rf /tmp".into(),
        },
        EventPayload::PermissionGranted {
            request_id: "req1".into(),
        },
        EventPayload::PermissionDenied {
            request_id: "req1".into(),
            reason: "destructive".into(),
        },
        EventPayload::TaskConfirmationRequested {
            request_id: "clarify1".into(),
            prompt: "Choose scope".into(),
            options: vec![TaskConfirmationOption {
                id: "tests".into(),
                label: "Tests only".into(),
                description: Some("Add tests first".into()),
            }],
            allow_multiple: false,
            allow_custom: true,
        },
        EventPayload::TaskConfirmationResolved {
            request_id: "clarify1".into(),
            selected_option_ids: vec!["tests".into()],
            custom_response: Some("Keep it focused".into()),
        },
        EventPayload::ToolInvocationStarted {
            invocation_id: "inv1".into(),
            tool_id: "shell.exec".into(),
            input_preview: String::new(),
        },
        EventPayload::ToolInvocationCompleted {
            invocation_id: "inv1".into(),
            tool_id: "shell.exec".into(),
            output_preview: "ok".into(),
            exit_code: Some(0),
            duration_ms: 120,
            truncated: false,
            images: vec![],
        },
        EventPayload::ToolInvocationFailed {
            invocation_id: "inv1".into(),
            tool_id: "shell.exec".into(),
            error: "not found".into(),
        },
        EventPayload::FilePatchProposed {
            patch_id: "p1".into(),
            diff: "--- a/foo\n+++ b/foo\n".into(),
        },
        EventPayload::FilePatchApplied {
            patch_id: "p1".into(),
        },
        EventPayload::MemoryProposed {
            memory_id: "mem1".into(),
            scope: "workspace".into(),
            key: Some("cmd".into()),
            content: "cargo build".into(),
        },
        EventPayload::MemoryAccepted {
            memory_id: "mem1".into(),
            scope: "user".into(),
            key: Some("lang".into()),
            content: "Rust".into(),
        },
        EventPayload::MemoryRejected {
            memory_id: "mem1".into(),
            reason: "stale".into(),
        },
        EventPayload::ReviewerFindingAdded {
            finding_id: "f1".into(),
            severity: "high".into(),
            message: "dangerous".into(),
        },
        EventPayload::AssistantMessageCompleted {
            message_id: "m2".into(),
            content: "done".into(),
        },
        EventPayload::AgentTaskCompleted {
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::AgentTaskFailed {
            task_id: agent_core::TaskId::new(),
            error: "timeout".into(),
        },
        EventPayload::TaskDecomposed {
            parent_task_id: agent_core::TaskId::new(),
            sub_task_ids: vec![agent_core::TaskId::new(), agent_core::TaskId::new()],
        },
        EventPayload::TaskBlocked {
            task_id: agent_core::TaskId::new(),
            blocking_task_id: agent_core::TaskId::new(),
            reason: "dep failed".into(),
        },
        EventPayload::AgentSpawned {
            agent_id: "agent_worker_1".into(),
            role: "Worker".into(),
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::AgentIdle {
            agent_id: "agent_worker_1".into(),
        },
        EventPayload::TaskRetried {
            task_id: agent_core::TaskId::new(),
            attempt: 1,
        },
        EventPayload::SessionCancelled {
            reason: "user stopped".into(),
        },
        EventPayload::SkillDiscovered {
            skill_id: "sk1".into(),
            name: "docs".into(),
            source: "builtin".into(),
        },
        EventPayload::SkillValidationFailed {
            path: "/tmp/bad/SKILL.md".into(),
            error: "invalid yaml".into(),
        },
        EventPayload::SkillActivated {
            skill_id: "sk1".into(),
            name: "docs".into(),
            source: "builtin".into(),
            activation_mode: "manual".into(),
        },
        EventPayload::SkillDeactivated {
            skill_id: "sk1".into(),
            name: "docs".into(),
            source: "builtin".into(),
        },
        EventPayload::SkillSuggested {
            skill_id: "sk1".into(),
            name: "docs".into(),
            reason: "user asked".into(),
        },
        EventPayload::McpServerStarting {
            server_id: "srv1".into(),
        },
        EventPayload::McpServerReady {
            server_id: "srv1".into(),
            tool_count: 5,
        },
        EventPayload::McpServerStopped {
            server_id: "srv1".into(),
        },
        EventPayload::McpServerFailed {
            server_id: "srv1".into(),
            error: "crash".into(),
        },
        EventPayload::McpToolCallStarted {
            server_id: "srv1".into(),
            tool_name: "search".into(),
        },
        EventPayload::McpToolCallCompleted {
            server_id: "srv1".into(),
            tool_name: "search".into(),
            duration_ms: 45,
        },
        EventPayload::McpTrustGranted {
            server_id: "srv1".into(),
        },
        EventPayload::McpTrustRevoked {
            server_id: "srv1".into(),
        },
        EventPayload::CatalogRefreshed {
            source: "builtin".into(),
            entry_count: 12,
        },
        EventPayload::CatalogEntryInstalling {
            catalog_id: "cat1".into(),
            source: "builtin".into(),
        },
        EventPayload::CatalogEntryInstalled {
            catalog_id: "cat1".into(),
            source: "builtin".into(),
            server_id: "srv1".into(),
        },
        EventPayload::CatalogEntryUninstalled {
            server_id: "srv1".into(),
        },
        EventPayload::CatalogRuntimeMissing {
            catalog_id: "cat1".into(),
            missing: vec!["node".into(), "python".into()],
        },
        EventPayload::CatalogSourceAdded {
            source: "mcp-registry".into(),
        },
        EventPayload::CatalogSourceFailed {
            source: "mcp-registry".into(),
            error: "timeout".into(),
        },
    ];

    let mut seen = HashSet::new();
    for variant in &variants {
        let et = variant.event_type();
        assert!(
            !et.is_empty(),
            "event_type() returned an empty string for a variant"
        );
        assert!(
            seen.insert(et.to_string()),
            "duplicate event_type string: '{et}'"
        );
    }

    // Total unique event_type strings should equal the number of variants.
    assert_eq!(
        seen.len(),
        variants.len(),
        "expected {} unique event_type strings, got {}",
        variants.len(),
        seen.len()
    );
}

// ── key-variant JSON roundtrip (standalone EventPayload, no envelope) ──────

#[test]
fn payload_serde_roundtrip_for_all_variants() {
    let variants: Vec<EventPayload> = vec![
        EventPayload::WorkspaceOpened {
            path: "/home/user/proj".into(),
        },
        EventPayload::SessionInitialized {
            model_profile: "opus".into(),
        },
        EventPayload::UserMessageAdded {
            message_id: "msg-42".into(),
            content: "explain this code".into(),
            display_content: None,
        },
        EventPayload::AgentTaskCreated {
            task_id: agent_core::TaskId::new(),
            title: "review PR".into(),
            role: AgentRole::Reviewer,
            dependencies: vec![],
        },
        EventPayload::AgentTaskStarted {
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::ContextAssembled { usage: usage() },
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::Threshold { ratio: 0.87 },
            before_tokens: 188_000,
            candidate_event_count: 42,
        },
        EventPayload::ContextCompactionCompleted {
            summary_id: "sum_done".into(),
            after_tokens: 12_000,
            fallback_used: false,
        },
        EventPayload::ContextCompactionFailed {
            error: "timeout".into(),
            fallback_used: true,
        },
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::NotEnoughHistory,
            ratio: 0.0,
        },
        EventPayload::CompactionSummary {
            summary_id: "sum_abc".into(),
            content: "## Key points\n- a\n- b".into(),
            replaces_event_range: (ts("2026-05-08T09:00:00Z"), ts("2026-05-08T10:00:00Z")),
            reason: CompactionReason::UserRequested,
            before_tokens: 188_000,
            after_tokens: 4_500,
            summarised_by_profile: "fast".into(),
        },
        EventPayload::ModelProfileSwitched {
            from_profile: "fast".into(),
            to_profile: "opus".into(),
            reasoning_effort: Some("high".into()),
            effective_at: ts("2026-05-09T10:00:00Z"),
            context_window: 200_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
        },
        EventPayload::ModelRequestStarted {
            model_profile: "fast".into(),
            model_id: "gpt-4.1-mini".into(),
        },
        EventPayload::ModelUsageRecorded {
            model_profile: "fast".into(),
            input_tokens: 123,
            output_tokens: 45,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(20),
        },
        EventPayload::ModelStreamStatus {
            phase: "stream_start".into(),
            retrying: true,
            retry_attempt: 1,
            max_retries: 1,
            message: "model stream start idle timeout; retrying".into(),
        },
        EventPayload::ModelTokenDelta {
            delta: "Hello".into(),
        },
        EventPayload::ModelToolCallRequested {
            tool_call_id: "call1".into(),
            tool_id: "shell.exec".into(),
        },
        EventPayload::PermissionRequested {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            preview: "rm -rf /tmp".into(),
        },
        EventPayload::PermissionGranted {
            request_id: "req1".into(),
        },
        EventPayload::PermissionDenied {
            request_id: "req1".into(),
            reason: "destructive".into(),
        },
        EventPayload::TaskConfirmationRequested {
            request_id: "clarify1".into(),
            prompt: "Choose scope".into(),
            options: vec![TaskConfirmationOption {
                id: "impl".into(),
                label: "Implementation".into(),
                description: None,
            }],
            allow_multiple: true,
            allow_custom: true,
        },
        EventPayload::TaskConfirmationResolved {
            request_id: "clarify1".into(),
            selected_option_ids: vec!["impl".into()],
            custom_response: None,
        },
        EventPayload::ToolInvocationStarted {
            invocation_id: "inv-1".into(),
            tool_id: "shell.exec".into(),
            input_preview: String::new(),
        },
        EventPayload::ToolInvocationCompleted {
            invocation_id: "inv-99".into(),
            tool_id: "fs.read".into(),
            output_preview: "line 1\nline 2".into(),
            exit_code: Some(0),
            duration_ms: 234,
            truncated: false,
            images: vec![],
        },
        EventPayload::ToolInvocationFailed {
            invocation_id: "inv-99".into(),
            tool_id: "fs.read".into(),
            error: "permission denied".into(),
        },
        EventPayload::FilePatchProposed {
            patch_id: "patch-1".into(),
            diff: "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-foo\n+bar\n".into(),
        },
        EventPayload::FilePatchApplied {
            patch_id: "patch-1".into(),
        },
        EventPayload::MemoryProposed {
            memory_id: "mem-7".into(),
            scope: "workspace".into(),
            key: Some("build.command".into()),
            content: "cargo build --release".into(),
        },
        EventPayload::MemoryAccepted {
            memory_id: "mem-7".into(),
            scope: "user".into(),
            key: Some("editor".into()),
            content: "vscode".into(),
        },
        EventPayload::MemoryRejected {
            memory_id: "mem-7".into(),
            reason: "stale".into(),
        },
        EventPayload::ReviewerFindingAdded {
            finding_id: "find-3".into(),
            severity: "medium".into(),
            message: "unwrap() used without context".into(),
        },
        EventPayload::AssistantMessageCompleted {
            message_id: "msg-42".into(),
            content: "The function computes...".into(),
        },
        EventPayload::AgentTaskCompleted {
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::AgentTaskFailed {
            task_id: agent_core::TaskId::new(),
            error: "max retries exceeded".into(),
        },
        EventPayload::TaskDecomposed {
            parent_task_id: agent_core::TaskId::new(),
            sub_task_ids: vec![agent_core::TaskId::new(), agent_core::TaskId::new()],
        },
        EventPayload::TaskBlocked {
            task_id: agent_core::TaskId::new(),
            blocking_task_id: agent_core::TaskId::new(),
            reason: "dep failed".into(),
        },
        EventPayload::AgentSpawned {
            agent_id: "agent_worker_bob".into(),
            role: "Worker".into(),
            task_id: agent_core::TaskId::new(),
        },
        EventPayload::AgentIdle {
            agent_id: "agent_worker_bob".into(),
        },
        EventPayload::TaskRetried {
            task_id: agent_core::TaskId::new(),
            attempt: 3,
        },
        EventPayload::SessionCancelled {
            reason: "user interrupt".into(),
        },
        EventPayload::SkillDiscovered {
            skill_id: "sk1".into(),
            name: "docs".into(),
            source: "builtin".into(),
        },
        EventPayload::SkillValidationFailed {
            path: "/tmp/bad/SKILL.md".into(),
            error: "missing required field 'name'".into(),
        },
        EventPayload::SkillActivated {
            skill_id: "code-review".into(),
            name: "Code Review".into(),
            source: "builtin".into(),
            activation_mode: "suggest".into(),
        },
        EventPayload::SkillDeactivated {
            skill_id: "code-review".into(),
            name: "Code Review".into(),
            source: "builtin".into(),
        },
        EventPayload::SkillSuggested {
            skill_id: "code-review".into(),
            name: "Code Review".into(),
            reason: "user asked for a review".into(),
        },
        EventPayload::McpServerStarting {
            server_id: "filesystem".into(),
        },
        EventPayload::McpServerReady {
            server_id: "filesystem".into(),
            tool_count: 3,
        },
        EventPayload::McpServerStopped {
            server_id: "filesystem".into(),
        },
        EventPayload::McpServerFailed {
            server_id: "bad-server".into(),
            error: "connection refused".into(),
        },
        EventPayload::McpToolCallStarted {
            server_id: "github".into(),
            tool_name: "create_issue".into(),
        },
        EventPayload::McpToolCallCompleted {
            server_id: "github".into(),
            tool_name: "create_issue".into(),
            duration_ms: 312,
        },
        EventPayload::McpTrustGranted {
            server_id: "filesystem".into(),
        },
        EventPayload::McpTrustRevoked {
            server_id: "filesystem".into(),
        },
        EventPayload::CatalogRefreshed {
            source: "mcp-registry".into(),
            entry_count: 47,
        },
        EventPayload::CatalogEntryInstalling {
            catalog_id: "cat1".into(),
            source: "builtin".into(),
        },
        EventPayload::CatalogEntryInstalled {
            catalog_id: "gh-modelcontextprotocol".into(),
            source: "mcp-registry".into(),
            server_id: "github-mcp".into(),
        },
        EventPayload::CatalogEntryUninstalled {
            server_id: "github-mcp".into(),
        },
        EventPayload::CatalogRuntimeMissing {
            catalog_id: "cat1".into(),
            missing: vec!["node".into(), "python".into()],
        },
        EventPayload::CatalogSourceAdded {
            source: "community-registry".into(),
        },
        EventPayload::CatalogSourceFailed {
            source: "community-registry".into(),
            error: "DNS resolution failed".into(),
        },
    ];

    for variant in &variants {
        let json = serde_json::to_string(variant).unwrap();
        let back: EventPayload = serde_json::from_str(&json).unwrap_or_else(|e| {
            panic!(
                "deserialization failed for variant '{}': {e}",
                variant.event_type()
            )
        });
        assert_eq!(
            variant,
            &back,
            "JSON roundtrip mismatch for variant '{}'",
            variant.event_type()
        );
    }
}
