# Kairox Ops Followups Batch 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce evaluation-time operational noise by filtering obvious durable memory-status proposals, triaging the remaining Dependabot `glib` advisory, and cleaning only confirmed-clean eval worktrees.

**Architecture:** Keep the runtime behavior change in `agent-runtime` with focused memory tests. Keep dependency advisory work separate from runtime changes because `glib` is currently a transitive Linux GUI stack dependency. Treat eval worktree cleanup as local maintenance with read-only audit evidence before any removal.

**Tech Stack:** Rust runtime memory handling, `agent-memory` markers, Cargo dependency graph tooling, Git worktree porcelain, GitHub Dependabot alerts.

---

## Batch Boundaries

- Lane J owns `crates/agent-runtime/src/memory_handler.rs` and `crates/agent-runtime/src/memory_handler_tests.rs`.
- Lane K owns dependency/advisory triage artifacts only. It must not change `memory_handler` files.
- Lane L owns local cleanup commands only. It must not edit, commit, push, or delete dirty eval worktrees.
- Lane J and Lane K can be developed independently. Lane L can run after Lane J/K worktrees are created or after they merge, but it must only target pre-existing `eval/*` worktrees marked clean by `scripts/audit-eval-worktrees.mjs --clean-only`.

## Task 1: Durable Memory Proposal Noise Filter

**Files:**

- Modify: `crates/agent-runtime/src/memory_handler.rs`
- Modify: `crates/agent-runtime/src/memory_handler_tests.rs`

- [ ] **Step 1: Add RED tests for obvious transient agent status proposals**

Add two tests to `crates/agent-runtime/src/memory_handler_tests.rs`.

The first test proves a durable workspace/user memory marker that only records agent-run status is not stored or proposed:

```rust
#[tokio::test]
async fn store_durable_agent_status_memory_marker_is_filtered() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, mut rx) = tokio::sync::broadcast::channel(16);
    let sqlite_mem = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(sqlite_mem.clone());
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        r#"<memory scope="workspace" key="task-status">Task completed: PR #1088 merged and CI passed.</memory>"#,
    )
    .await;

    assert!(
        matches!(rx.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Empty)),
        "filtered durable status memory must not emit a memory event"
    );

    let all = sqlite_mem
        .query_including_pending(agent_memory::MemoryQuery {
            scope: None,
            keywords: Vec::new(),
            limit: 10,
            session_id: None,
            workspace_id: None,
            branch: None,
        })
        .await
        .unwrap();
    assert!(
        all.iter().all(|m| m.content != "Task completed: PR #1088 merged and CI passed."),
        "filtered durable status memory must not be stored"
    );
}
```

The second test locks the product decision for session-scoped completion notes: session scope is still temporary and auto-accepted, even if the key/content look like task status.

```rust
#[tokio::test]
async fn store_session_agent_status_memory_marker_is_still_accepted() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, mut rx) = tokio::sync::broadcast::channel(16);
    let sqlite_mem = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(sqlite_mem.clone());
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        r#"<memory scope="session" key="task-status">Task completed: local test run passed.</memory>"#,
    )
    .await;

    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event.payload, EventPayload::MemoryAccepted { ref scope, .. } if scope == "session"),
        "session status memory remains temporary accepted state, got: {:?}",
        event.payload
    );
}
```

Expected RED:

```bash
cargo test -p agent-runtime store_durable_agent_status_memory_marker_is_filtered
```

The durable status test fails because current code stores the marker and emits `MemoryProposed`.

- [ ] **Step 2: Implement a conservative durable-only filter**

Add a small helper in `memory_handler.rs` near the storage functions:

```rust
fn is_transient_agent_status_memory(entry: &MemoryEntry) -> bool {
    if !durable_memory_requires_confirmation(&entry.scope) {
        return false;
    }

    let key = entry.key.as_deref().map(normalize_memory_signal);
    if matches!(
        key.as_deref(),
        Some(
            "task-status"
                | "task-result"
                | "task-summary"
                | "run-status"
                | "run-summary"
                | "evaluation-result"
                | "evaluation-summary"
                | "pr-status"
                | "ci-status"
                | "completion-status"
        )
    ) {
        return true;
    }

    let content = normalize_memory_signal(&entry.content);
    [
        "task completed:",
        "task complete:",
        "completed task:",
        "run completed:",
        "evaluation result:",
        "merged pr",
        "merged pull request",
        "pr #",
        "pull request #",
        "ci passed",
        "tests passed",
        "validation passed",
    ]
    .iter()
    .any(|prefix| content.starts_with(prefix))
}

fn normalize_memory_signal(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

Then, inside `store_memory_markers_with_branch`, after `requires_confirmation` is computed and before storing durable proposals:

```rust
if is_transient_agent_status_memory(&entry) {
    continue;
}
```

Do not emit `MemoryRejected` for filtered status noise. The goal is to prevent durable memory proposal noise in traces and memory approval UI, not to add a new event stream.

- [ ] **Step 3: Preserve normal durable memory proposals**

Run or add focused coverage proving existing durable memories still propose normally:

```bash
cargo test -p agent-runtime store_user_scope_marker_produces_proposed_event
cargo test -p agent-runtime workspace_scope_memory_is_proposed_even_in_autonomous_mode
```

If either test fails, the filter is too broad. Narrow key/content matching instead of changing the expected behavior for user/workspace preference memories.

- [ ] **Step 4: Verify Lane J**

Run:

```bash
cargo fmt --all
cargo test -p agent-runtime store_durable_agent_status_memory_marker_is_filtered
cargo test -p agent-runtime store_session_agent_status_memory_marker_is_still_accepted
cargo test -p agent-runtime store_user_scope_marker_produces_proposed_event
cargo test -p agent-runtime memory_protocol
cargo fmt --all --check
cargo clippy -p agent-runtime --all-targets -- -D warnings
git diff --check
```

Dev App verification requirement: this is runtime behavior that changes memory proposal visibility. If a local Dev App run is practical, start with `bash scripts/dev-pilot.sh`, trigger a fake-model memory marker path, and verify no durable status proposal appears. If pilot is blocked, record the exact blocker and use the Rust memory protocol tests as the fallback evidence.

## Task 2: Dependabot `glib` Advisory Triage

**Files:**

- Create if the alert remains unfixable by compatible updates: `docs/security/dependabot-glib-ghsa-wrw7-89jp-8q8g.md`
- Modify if a compatible update exists: `Cargo.lock`
- Modify only if required by a safe compatible update: relevant `Cargo.toml`

- [ ] **Step 1: Reconfirm the alert and dependency path**

Run:

```bash
gh api repos/Z-Only/kairox/dependabot/alerts/5 --jq '{number,state,package:.dependency.package.name,manifest:.dependency.manifest_path,severity:.security_vulnerability.severity,range:.security_vulnerability.vulnerable_version_range,patched:.security_vulnerability.first_patched_version.identifier,advisory:.security_advisory.ghsa_id}'
cargo tree --target all -i glib@0.18.5
cargo update -p glib --dry-run
cargo update -p tauri --dry-run --verbose
```

Expected current evidence: alert #5 is open for `glib` `<0.20.0`; `glib 0.18.5` is pulled through Tauri's Linux GTK stack; direct `glib`/`tauri` lockfile updates do not move to the patched `glib 0.20.0` line.

- [ ] **Step 2: Attempt only compatible update paths**

If `cargo update -p tauri --dry-run --verbose` shows a compatible Tauri/wry/gtk stack update that reaches `glib >= 0.20.0`, apply it:

```bash
cargo update -p tauri
cargo tree --target all -i glib
```

If no compatible update exists, do not force an incompatible GTK stack upgrade in the same PR. Create the triage document instead, including:

- alert number and advisory id;
- vulnerable package and manifest;
- current dependency path from `cargo tree --target all -i glib@0.18.5`;
- commands tried and their outcome;
- why no lockfile-only fix is available;
- follow-up trigger: re-run when Tauri/wry/GTK releases a compatible stack using `glib >= 0.20.0`.

- [ ] **Step 3: Verify Lane K**

For a lockfile update, run:

```bash
cargo fmt --all --check
cargo check -p agent-gui-tauri --all-targets
git diff --check
```

For triage-doc-only output, run:

```bash
bun run format:check
git diff --check
```

Dev App verification can be skipped for triage-doc-only output because no runtime or GUI behavior changes. If the lockfile changes GUI stack dependencies, run Dev App verification with `bash scripts/dev-pilot.sh` or record the exact local blocker.

## Task 3: Local Clean Eval Worktree Cleanup

**Files:**

- No versioned file changes.

- [ ] **Step 1: Capture current audit**

Run from the main checkout:

```bash
node scripts/audit-eval-worktrees.mjs --json
node scripts/audit-eval-worktrees.mjs --clean-only
node scripts/audit-eval-worktrees.mjs --dirty-only
```

Expected current shape: clean and dirty eval worktrees are listed separately. Dirty worktrees are preserved.

- [ ] **Step 2: Remove only clean eval worktrees**

For each path listed by `--clean-only`, confirm `git status --short --branch` is clean, then remove from the main checkout:

```bash
git worktree remove <clean-eval-worktree-path>
git branch -D <matching-eval-branch>
```

Do not use `--force` unless `git status --short` is empty and normal removal fails for a stale administrative reason. Do not delete branches/worktrees reported by `--dirty-only`.

- [ ] **Step 3: Verify cleanup**

Run:

```bash
node scripts/audit-eval-worktrees.mjs --summary
git worktree list --porcelain
git status --short --branch
```

Expected: the clean eval worktrees targeted in Step 2 are gone; dirty eval worktrees remain visible for later manual review; main checkout is clean.

## Final Batch Gates

- Lane J needs TDD RED/GREEN evidence, Rust format, focused runtime tests, clippy, PR creation, CI observation until merge, and cleanup.
- Lane K either produces a small lockfile/update PR with GUI-stack verification or a triage-doc PR with explicit evidence that no compatible update path currently exists.
- Lane L is local maintenance only and must not be represented as a code PR. It must not delete dirty eval worktrees.
- Main branch CodeQL should be checked after batch PR merges; do not declare the Dependabot advisory fixed unless the alert closes or `Cargo.lock` no longer contains a vulnerable `glib` version.
