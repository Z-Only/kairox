use super::*;
use crate::ids::{AgentId, SessionId, WorkspaceId};

#[test]
fn serializes_user_message_event_with_required_envelope_fields() {
    use chrono::TimeZone;
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: "msg-user-1".into(),
            content: "explain the repo".into(),
            display_content: None,
        },
    )
    .with_timestamp(chrono::Utc.with_ymd_and_hms(2026, 4, 29, 2, 0, 0).unwrap());

    let json = serde_json::to_value(&event).unwrap();

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["event_type"], "UserMessageAdded");
    assert_eq!(json["privacy"], "full_trace");
    assert_eq!(json["timestamp"], "2026-04-29T02:00:00Z");
    assert_eq!(json["source_agent_id"], "agent_system");
    assert_eq!(json["payload"]["content"], "explain the repo");
    assert!(json["workspace_id"].as_str().unwrap().starts_with("wrk_"));
    assert!(json["session_id"].as_str().unwrap().starts_with("ses_"));
}

// MCP event tests
#[test]
fn mcp_server_starting_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerStarting {
            server_id: "test".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerStarting");
    assert_eq!(json["payload"]["server_id"], "test");
}

#[test]
fn mcp_server_ready_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerReady {
            server_id: "fs".into(),
            tool_count: 5,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerReady");
    assert_eq!(json["payload"]["tool_count"], 5);
}

#[test]
fn mcp_server_failed_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerFailed {
            server_id: "bad".into(),
            error: "crashed".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerFailed");
    assert_eq!(json["payload"]["error"], "crashed");
}

#[test]
fn mcp_tool_call_completed_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpToolCallCompleted {
            server_id: "github".into(),
            tool_name: "create_issue".into(),
            duration_ms: 150,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpToolCallCompleted");
    assert_eq!(json["payload"]["duration_ms"], 150);
}

#[test]
fn mcp_trust_events_serialize() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpTrustGranted {
            server_id: "fs".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpTrustGranted");

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpTrustRevoked {
            server_id: "fs".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpTrustRevoked");
}

#[test]
fn catalog_source_added_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::CatalogSourceAdded {
            source: "mcp-registry".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "CatalogSourceAdded");
    assert_eq!(json["payload"]["type"], "CatalogSourceAdded");
    assert_eq!(json["payload"]["source"], "mcp-registry");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(
        matches!(back, EventPayload::CatalogSourceAdded { ref source } if source == "mcp-registry")
    );
}

#[test]
fn catalog_source_failed_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::CatalogSourceFailed {
            source: "mcp-registry".into(),
            error: "timeout".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "CatalogSourceFailed");
    assert_eq!(json["payload"]["type"], "CatalogSourceFailed");
    assert_eq!(json["payload"]["error"], "timeout");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(
        matches!(back, EventPayload::CatalogSourceFailed { ref source, ref error }
        if source == "mcp-registry" && error == "timeout")
    );
}

#[test]
fn compaction_reason_serializes_with_internal_tag() {
    let r = CompactionReason::UserRequested;
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["type"], "UserRequested");

    let r = CompactionReason::Threshold { ratio: 0.87 };
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["type"], "Threshold");
    assert!((json["ratio"].as_f64().unwrap() - 0.87).abs() < 1e-6);
}

#[test]
fn context_compaction_started_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::Threshold { ratio: 0.9 },
            before_tokens: 180_000,
            candidate_event_count: 42,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["before_tokens"], 180_000);
    assert_eq!(json["payload"]["candidate_event_count"], 42);
    assert_eq!(json["payload"]["reason"]["type"], "Threshold");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(matches!(
        back,
        EventPayload::ContextCompactionStarted { .. }
    ));
}

#[test]
fn context_compaction_completed_and_failed_round_trip() {
    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "sum_1".into(),
        after_tokens: 30_000,
        fallback_used: false,
    };
    let json = serde_json::to_value(&completed).unwrap();
    assert_eq!(json["type"], "ContextCompactionCompleted");
    assert_eq!(json["fallback_used"], false);
    let _back: EventPayload = serde_json::from_value(json).unwrap();

    let failed = EventPayload::ContextCompactionFailed {
        error: "model timeout".into(),
        fallback_used: true,
    };
    let json = serde_json::to_value(&failed).unwrap();
    assert_eq!(json["type"], "ContextCompactionFailed");
    assert_eq!(json["fallback_used"], true);
    let _back: EventPayload = serde_json::from_value(json).unwrap();
}

#[test]
fn compaction_summary_event_round_trips_with_timestamp_range() {
    use chrono::TimeZone;
    let from = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 9, 0, 0).unwrap();
    let to = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
    let payload = EventPayload::CompactionSummary {
        summary_id: "sum_1".into(),
        content: "## User goal\n...".into(),
        replaces_event_range: (from, to),
        reason: CompactionReason::UserRequested,
        before_tokens: 180_000,
        after_tokens: 4_000,
        summarised_by_profile: "fast".into(),
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "CompactionSummary");
    assert_eq!(json["summarised_by_profile"], "fast");
    let back: EventPayload = serde_json::from_value(json).unwrap();
    if let EventPayload::CompactionSummary {
        replaces_event_range,
        ..
    } = back
    {
        assert_eq!(replaces_event_range.0, from);
        assert_eq!(replaces_event_range.1, to);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn event_type_method_covers_new_compaction_variants() {
    let started = EventPayload::ContextCompactionStarted {
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        candidate_event_count: 0,
    };
    assert_eq!(started.event_type(), "ContextCompactionStarted");

    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "x".into(),
        after_tokens: 0,
        fallback_used: false,
    };
    assert_eq!(completed.event_type(), "ContextCompactionCompleted");

    let failed = EventPayload::ContextCompactionFailed {
        error: "x".into(),
        fallback_used: false,
    };
    assert_eq!(failed.event_type(), "ContextCompactionFailed");

    let summary = EventPayload::CompactionSummary {
        summary_id: "x".into(),
        content: "x".into(),
        replaces_event_range: (chrono::Utc::now(), chrono::Utc::now()),
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        after_tokens: 0,
        summarised_by_profile: "fast".into(),
    };
    assert_eq!(summary.event_type(), "CompactionSummary");
}

#[test]
fn context_compaction_skipped_round_trips() {
    let payload = EventPayload::ContextCompactionSkipped {
        reason: CompactionSkipReason::AlreadyCompacting,
        ratio: 0.92,
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "ContextCompactionSkipped");
    assert_eq!(json["reason"]["type"], "AlreadyCompacting");
    assert!((json["ratio"].as_f64().unwrap() - 0.92).abs() < 1e-6);
    let back: EventPayload = serde_json::from_value(json).unwrap();
    assert!(matches!(
        back,
        EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::AlreadyCompacting,
            ..
        }
    ));

    let disabled = EventPayload::ContextCompactionSkipped {
        reason: CompactionSkipReason::ThresholdDisabled,
        ratio: 0.5,
    };
    let json = serde_json::to_value(&disabled).unwrap();
    assert_eq!(json["reason"]["type"], "ThresholdDisabled");
    assert_eq!(disabled.event_type(), "ContextCompactionSkipped");
}

#[test]
fn context_assembled_payload_carries_usage_struct() {
    use crate::context_types::{ContextSource, ContextUsage};

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: ContextUsage {
                total_tokens: 12_345,
                budget_tokens: 188_000,
                context_window: 200_000,
                output_reservation: 12_000,
                by_source: vec![
                    (ContextSource::System, 800),
                    (ContextSource::ToolDefinitions, 11_545),
                ],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            },
        },
    );

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextAssembled");
    assert_eq!(json["payload"]["usage"]["total_tokens"], 12_345);
    assert_eq!(json["payload"]["usage"]["context_window"], 200_000);
    assert_eq!(json["payload"]["usage"]["estimator"], "cl100k_base");
    assert_eq!(json["payload"]["usage"]["by_source"][0][0], "system");
}

#[test]
fn model_profile_switched_event_round_trips() {
    use chrono::TimeZone;
    let effective_at = chrono::Utc.with_ymd_and_hms(2026, 5, 9, 10, 0, 0).unwrap();
    let payload = EventPayload::ModelProfileSwitched {
        from_profile: "fast".into(),
        to_profile: "claude-opus".into(),
        reasoning_effort: Some("high".into()),
        effective_at,
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry".into(),
    };

    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "ModelProfileSwitched");
    assert_eq!(json["from_profile"], "fast");
    assert_eq!(json["to_profile"], "claude-opus");
    assert_eq!(json["reasoning_effort"], "high");
    assert_eq!(json["context_window"], 200_000);
    assert_eq!(json["output_limit"], 16_384);
    assert_eq!(json["limit_source"], "builtin_registry");
    assert_eq!(json["effective_at"], "2026-05-09T10:00:00Z");

    let back: EventPayload = serde_json::from_value(json).unwrap();
    match back {
        EventPayload::ModelProfileSwitched {
            from_profile,
            to_profile,
            reasoning_effort,
            effective_at: at,
            context_window,
            output_limit,
            limit_source,
        } => {
            assert_eq!(from_profile, "fast");
            assert_eq!(to_profile, "claude-opus");
            assert_eq!(reasoning_effort.as_deref(), Some("high"));
            assert_eq!(at, effective_at);
            assert_eq!(context_window, 200_000);
            assert_eq!(output_limit, 16_384);
            assert_eq!(limit_source, "builtin_registry");
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn event_type_method_covers_model_profile_switched() {
    let p = EventPayload::ModelProfileSwitched {
        from_profile: "a".into(),
        to_profile: "b".into(),
        reasoning_effort: None,
        effective_at: chrono::Utc::now(),
        context_window: 0,
        output_limit: 0,
        limit_source: "fallback".into(),
    };
    assert_eq!(p.event_type(), "ModelProfileSwitched");
}

#[test]
fn monitor_stop_reason_serializes_with_internal_tag() {
    let exit = MonitorStopReason::ExitCode { code: 1 };
    let json = serde_json::to_value(exit).unwrap();
    assert_eq!(json["type"], "ExitCode");
    assert_eq!(json["code"], 1);

    let timeout = MonitorStopReason::Timeout;
    let json = serde_json::to_value(timeout).unwrap();
    assert_eq!(json["type"], "Timeout");

    let user = MonitorStopReason::UserStopped;
    let json = serde_json::to_value(user).unwrap();
    assert_eq!(json["type"], "UserStopped");

    let session = MonitorStopReason::SessionEnded;
    let json = serde_json::to_value(session).unwrap();
    assert_eq!(json["type"], "SessionEnded");
}

#[test]
fn monitor_started_event_round_trips() {
    let payload = EventPayload::MonitorStarted {
        monitor_id: "mon_1".into(),
        description: "watch build logs".into(),
        command: "tail -f build.log".into(),
        persistent: false,
        timeout_ms: 300_000,
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "MonitorStarted");
    assert_eq!(json["monitor_id"], "mon_1");
    assert_eq!(json["persistent"], false);
    assert_eq!(json["timeout_ms"], 300_000);
    let back: EventPayload = serde_json::from_value(json).unwrap();
    assert!(
        matches!(back, EventPayload::MonitorStarted { ref monitor_id, .. } if monitor_id == "mon_1")
    );
}

#[test]
fn monitor_event_round_trips() {
    let payload = EventPayload::MonitorEvent {
        monitor_id: "mon_1".into(),
        line: "ERROR: connection refused".into(),
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "MonitorEvent");
    assert_eq!(json["line"], "ERROR: connection refused");
    let back: EventPayload = serde_json::from_value(json).unwrap();
    assert!(matches!(back, EventPayload::MonitorEvent { .. }));
}

#[test]
fn monitor_stopped_event_round_trips() {
    let payload = EventPayload::MonitorStopped {
        monitor_id: "mon_1".into(),
        reason: MonitorStopReason::ExitCode { code: 0 },
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "MonitorStopped");
    assert_eq!(json["reason"]["type"], "ExitCode");
    assert_eq!(json["reason"]["code"], 0);
    let back: EventPayload = serde_json::from_value(json).unwrap();
    match back {
        EventPayload::MonitorStopped { reason, .. } => {
            assert_eq!(reason, MonitorStopReason::ExitCode { code: 0 });
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn monitor_failed_event_round_trips() {
    let payload = EventPayload::MonitorFailed {
        monitor_id: "mon_1".into(),
        error: "spawn failed".into(),
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "MonitorFailed");
    assert_eq!(json["error"], "spawn failed");
    let back: EventPayload = serde_json::from_value(json).unwrap();
    assert!(
        matches!(back, EventPayload::MonitorFailed { ref error, .. } if error == "spawn failed")
    );
}

#[test]
fn event_type_method_covers_monitor_variants() {
    assert_eq!(
        EventPayload::MonitorStarted {
            monitor_id: "x".into(),
            description: "x".into(),
            command: "x".into(),
            persistent: false,
            timeout_ms: 0,
        }
        .event_type(),
        "MonitorStarted"
    );
    assert_eq!(
        EventPayload::MonitorEvent {
            monitor_id: "x".into(),
            line: "x".into(),
        }
        .event_type(),
        "MonitorEvent"
    );
    assert_eq!(
        EventPayload::MonitorStopped {
            monitor_id: "x".into(),
            reason: MonitorStopReason::Timeout,
        }
        .event_type(),
        "MonitorStopped"
    );
    assert_eq!(
        EventPayload::MonitorFailed {
            monitor_id: "x".into(),
            error: "x".into(),
        }
        .event_type(),
        "MonitorFailed"
    );
}
mod serde_roundtrip {
    use super::*;
    use crate::AutonomousTaskId;

    /// Helper: serialize to JSON string and deserialize back, asserting equality.
    fn roundtrip(payload: &EventPayload) {
        let json = serde_json::to_string(payload).unwrap();
        let decoded: EventPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(*payload, decoded, "round-trip failed for: {json}");
    }

    #[test]
    fn context_compaction_skipped_already_compacting() {
        roundtrip(&EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::AlreadyCompacting,
            ratio: 0.85,
        });
    }

    #[test]
    fn context_compaction_skipped_threshold_disabled() {
        roundtrip(&EventPayload::ContextCompactionSkipped {
            reason: CompactionSkipReason::ThresholdDisabled,
            ratio: 0.0,
        });
    }

    #[test]
    fn tool_invocation_completed_with_images() {
        roundtrip(&EventPayload::ToolInvocationCompleted {
            invocation_id: "inv-1".into(),
            tool_id: "browser.action".into(),
            output_preview: "screenshot taken".into(),
            exit_code: Some(0),
            duration_ms: 1500,
            truncated: false,
            images: vec![ImageAttachment {
                media_type: "image/png".into(),
                data: "iVBORw0KGgo=".into(),
                label: Some("screenshot".into()),
            }],
        });
    }

    #[test]
    fn tool_invocation_completed_empty_images() {
        roundtrip(&EventPayload::ToolInvocationCompleted {
            invocation_id: "inv-2".into(),
            tool_id: "shell.exec".into(),
            output_preview: "ok".into(),
            exit_code: Some(0),
            duration_ms: 42,
            truncated: false,
            images: vec![],
        });
    }

    #[test]
    fn tool_invocation_completed_image_without_label() {
        roundtrip(&EventPayload::ToolInvocationCompleted {
            invocation_id: "inv-3".into(),
            tool_id: "computer.use".into(),
            output_preview: "done".into(),
            exit_code: None,
            duration_ms: 800,
            truncated: true,
            images: vec![ImageAttachment {
                media_type: "image/jpeg".into(),
                data: "/9j/4AAQ".into(),
                label: None,
            }],
        });
    }

    #[test]
    fn autonomous_task_created() {
        roundtrip(&EventPayload::AutonomousTaskCreated {
            autonomous_task_id: AutonomousTaskId::new(),
            goal: "refactor auth module".into(),
            acceptance_criteria: vec!["tests pass".into(), "no regressions".into()],
            max_sessions: 3,
        });
    }

    #[test]
    fn autonomous_task_session_started() {
        roundtrip(&EventPayload::AutonomousTaskSessionStarted {
            autonomous_task_id: AutonomousTaskId::new(),
            session_id: SessionId::new(),
            session_index: 0,
        });
    }

    #[test]
    fn autonomous_task_checkpointed() {
        roundtrip(&EventPayload::AutonomousTaskCheckpointed {
            autonomous_task_id: AutonomousTaskId::new(),
            session_id: SessionId::new(),
            session_index: 1,
            checkpoint_json: r#"{"progress":50}"#.into(),
            end_reason: "budget_exhausted".into(),
        });
    }

    #[test]
    fn autonomous_task_completed() {
        roundtrip(&EventPayload::AutonomousTaskCompleted {
            autonomous_task_id: AutonomousTaskId::new(),
            total_sessions: 2,
        });
    }

    #[test]
    fn autonomous_task_failed() {
        roundtrip(&EventPayload::AutonomousTaskFailed {
            autonomous_task_id: AutonomousTaskId::new(),
            reason: "max retries exceeded".into(),
        });
    }

    #[test]
    fn autonomous_task_cancelled() {
        roundtrip(&EventPayload::AutonomousTaskCancelled {
            autonomous_task_id: AutonomousTaskId::new(),
        });
    }
}

mod image_attachment_tests {
    use super::*;

    #[test]
    fn construction_with_label() {
        let attachment = ImageAttachment {
            media_type: "image/png".into(),
            data: "iVBORw0KGgo=".into(),
            label: Some("screenshot".into()),
        };
        assert_eq!(attachment.media_type, "image/png");
        assert_eq!(attachment.data, "iVBORw0KGgo=");
        assert_eq!(attachment.label, Some("screenshot".into()));
    }

    #[test]
    fn construction_without_label() {
        let attachment = ImageAttachment {
            media_type: "image/jpeg".into(),
            data: "/9j/4AAQ".into(),
            label: None,
        };
        assert_eq!(attachment.media_type, "image/jpeg");
        assert!(attachment.label.is_none());
    }

    #[test]
    fn serde_roundtrip_with_label() {
        let attachment = ImageAttachment {
            media_type: "image/png".into(),
            data: "iVBORw0KGgo=".into(),
            label: Some("screenshot".into()),
        };
        let json = serde_json::to_string(&attachment).unwrap();
        let decoded: ImageAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(attachment, decoded);

        // Verify label is present in serialized output
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["label"], "screenshot");
    }

    #[test]
    fn serde_roundtrip_without_label() {
        let attachment = ImageAttachment {
            media_type: "image/jpeg".into(),
            data: "/9j/4AAQ".into(),
            label: None,
        };
        let json = serde_json::to_string(&attachment).unwrap();
        let decoded: ImageAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(attachment, decoded);

        // Verify label is omitted (skip_serializing_if = "Option::is_none")
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("label").is_none());
    }

    #[test]
    fn deserialize_with_missing_label_defaults_to_none() {
        let json = r#"{"media_type":"image/png","data":"abc123"}"#;
        let attachment: ImageAttachment = serde_json::from_str(json).unwrap();
        assert_eq!(attachment.media_type, "image/png");
        assert_eq!(attachment.data, "abc123");
        assert!(attachment.label.is_none());
    }

    #[test]
    fn equality_same_fields() {
        let a = ImageAttachment {
            media_type: "image/png".into(),
            data: "data1".into(),
            label: Some("l".into()),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_data() {
        let a = ImageAttachment {
            media_type: "image/png".into(),
            data: "data1".into(),
            label: None,
        };
        let b = ImageAttachment {
            media_type: "image/png".into(),
            data: "data2".into(),
            label: None,
        };
        assert_ne!(a, b);
    }
}
