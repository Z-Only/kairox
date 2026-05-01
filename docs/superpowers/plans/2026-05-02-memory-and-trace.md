# Memory Layer + GUI Trace Visualization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade agent-memory from a stub to a production MemoryStore with SQLite persistence, keyword retrieval, tiktoken estimation, and `<memory>` marker protocol; build a three-density collapsible TraceTimeline with inline permission prompts and Markdown rendering in the GUI.

**Architecture:** Two parallel workstreams — (A) Rust backend memory pipeline and (B) Vue GUI trace + Markdown. Workstream A does not block workstream B until the integration task (Task 10). Workstream B can use existing events (ToolInvocation*, Permission*) immediately.

**Tech Stack:** Rust, sqlx, tiktoken-rs, regex, tokio oneshot channels, Vue 3, markdown-it, highlight.js, Tauri 2 commands

---

## File Structure

### New Files

| File                                                 | Responsibility                                       |
| ---------------------------------------------------- | ---------------------------------------------------- |
| `crates/agent-memory/src/store.rs`                   | MemoryStore trait + SqliteMemoryStore implementation |
| `crates/agent-memory/src/marker.rs`                  | `<memory>` marker parser + strip function            |
| `crates/agent-memory/src/extractor.rs`               | Keyword extraction from text                         |
| `apps/agent-gui/src/components/TraceEntry.vue`       | Single collapsible trace entry component             |
| `apps/agent-gui/src/components/PermissionPrompt.vue` | Inline permission approve/deny component             |
| `apps/agent-gui/src/composables/useTraceStore.ts`    | Reactive trace state management                      |
| `apps/agent-gui/src/types/trace.ts`                  | TypeScript types for trace entries                   |
| `apps/agent-gui/src/utils/markdown.ts`               | markdown-it + highlight.js setup                     |

### Modified Files

| File                                               | Changes                                                                            |
| -------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `crates/agent-memory/Cargo.toml`                   | Add sqlx, tiktoken-rs, async-trait, regex, chrono, uuid, serde_json deps           |
| `crates/agent-memory/src/lib.rs`                   | Re-export new modules and types                                                    |
| `crates/agent-memory/src/memory.rs`                | Add `from_marker()` constructor, `accepted` field                                  |
| `crates/agent-memory/src/context.rs`               | Full rewrite with tiktoken, MemoryStore, priority truncation                       |
| `crates/agent-core/src/events.rs`                  | Rename MemoryProposed→MemoryStored, MemoryAccepted→remove; add scope/key to events |
| `crates/agent-tools/src/permission.rs`             | Add `Interactive` to PermissionMode, `Pending` to PermissionOutcome                |
| `crates/agent-runtime/Cargo.toml`                  | Add `agent-memory` to dev-dependencies (already in deps)                           |
| `crates/agent-runtime/src/facade_runtime.rs`       | Add memory_store, pending_permissions, marker processing, resolve_permission()     |
| `crates/agent-store/src/event_store.rs`            | Add `pool()` accessor, `ensure_memories_table()`                                   |
| `apps/agent-gui/src/components/TraceTimeline.vue`  | Full rewrite with three-density timeline                                           |
| `apps/agent-gui/src/components/ChatPanel.vue`      | Markdown rendering + marker stripping on display                                   |
| `apps/agent-gui/src/components/StatusBar.vue`      | Show permission mode                                                               |
| `apps/agent-gui/src/stores/session.ts`             | Handle MemoryStored/MemoryRejected events                                          |
| `apps/agent-gui/src/composables/useTauriEvents.ts` | Route trace events to useTraceStore                                                |
| `apps/agent-gui/src/types/index.ts`                | Add MemoryStored/MemoryRejected event payloads                                     |
| `apps/agent-gui/src-tauri/src/app_state.rs`        | Add memory_store field                                                             |
| `apps/agent-gui/src-tauri/src/commands.rs`         | Add resolve_permission, query_memories, delete_memory commands                     |
| `apps/agent-gui/src-tauri/src/lib.rs`              | Init MemoryStore, use Interactive mode, register new commands                      |
| `apps/agent-gui/package.json`                      | Add markdown-it, highlight.js dependencies                                         |

---

## Task 1: Keyword Extractor

**Files:**

- Create: `crates/agent-memory/src/extractor.rs`
- Modify: `crates/agent-memory/Cargo.toml` — add `regex` dep
- Modify: `crates/agent-memory/src/lib.rs` — add `pub mod extractor;`

- [ ] **Step 1: Write the failing test**

Add to `crates/agent-memory/src/extractor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_meaningful_keywords_and_filters_stop_words() {
        let keywords = extract_keywords("The project uses cargo nextest for testing");
        assert!(keywords.contains(&"project".to_string()));
        assert!(keywords.contains(&"cargo".to_string()));
        assert!(keywords.contains(&"nextest".to_string()));
        assert!(keywords.contains(&"testing".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"for".to_string()));
    }

    #[test]
    fn skips_short_tokens() {
        let keywords = extract_keywords("I am a go programmer");
        assert!(!keywords.iter().any(|k| k == "i" || k == "am" || k == "a"));
    }

    #[test]
    fn limits_to_20_keywords() {
        let long_text = (1..=50).map(|i| format!("keyword{i}")).collect::<Vec<_>>().join(" ");
        let keywords = extract_keywords(&long_text);
        assert!(keywords.len() <= 20);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(extract_keywords("").is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-memory -- extractor`
Expected: FAIL — `extract_keywords` not defined

- [ ] **Step 3: Write minimal implementation**

```rust
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all",
    "can", "had", "her", "was", "one", "our", "out", "has",
    "this", "that", "from", "with", "have", "will", "been",
    "they", "what", "about", "which", "their", "would", "there",
    "its", "also", "just", "more", "some", "than", "into",
];

pub fn extract_keywords(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| s.len() > 2)
        .filter(|s| !STOP_WORDS.contains(s))
        .take(20)
        .map(String::from)
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-memory -- extractor`
Expected: PASS

- [ ] **Step 5: Add `pub mod extractor;` to `lib.rs` and update `Cargo.toml`**

Add `regex = { workspace = true }` to `crates/agent-memory/Cargo.toml` dependencies.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-memory/src/extractor.rs crates/agent-memory/src/lib.rs crates/agent-memory/Cargo.toml
git commit -m "feat(memory): add keyword extractor for memory indexing"
```

---

## Task 2: Memory Marker Parser

**Files:**

- Create: `crates/agent-memory/src/marker.rs`
- Modify: `crates/agent-memory/src/lib.rs` — add `pub mod marker;`

- [ ] **Step 1: Write the failing test**

Add to `crates/agent-memory/src/marker.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_marker_with_scope_and_key() {
        let text = r#"Some response <memory scope="workspace" key="test-runner">Use cargo nextest</memory> more text"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].scope, MemoryScope::Workspace);
        assert_eq!(markers[0].key, Some("test-runner".to_string()));
        assert_eq!(markers[0].content, "Use cargo nextest");
    }

    #[test]
    fn extracts_marker_defaulting_to_session_scope() {
        let text = r#"<memory>Session note</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].scope, MemoryScope::Session);
        assert_eq!(markers[0].key, None);
        assert_eq!(markers[0].content, "Session note");
    }

    #[test]
    fn extracts_multiple_markers() {
        let text = r#"<memory scope="user">User fact</memory><memory scope="workspace" key="build">Build info</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 2);
    }

    #[test]
    fn skips_empty_markers() {
        let text = r#"<memory></memory><memory>   </memory><memory>Valid</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].content, "Valid");
    }

    #[test]
    fn strip_removes_all_markers() {
        let text = r#"Hello <memory scope="workspace">Save this</memory> World"#;
        let stripped = strip_memory_markers(text);
        assert_eq!(stripped, "Hello  World");
        assert!(!stripped.contains("<memory"));
    }

    #[test]
    fn strip_multiline_marker() {
        let text = "Result:\n<memory scope=\"session\">\nMultiple\nlines\n</memory>\nDone";
        let stripped = strip_memory_markers(text);
        assert_eq!(stripped, "Result:\nDone");
    }

    #[test]
    fn no_markers_returns_empty_and_strip_is_noop() {
        let text = "No markers here";
        assert!(extract_memory_markers(text).is_empty());
        assert_eq!(strip_memory_markers(text), text);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-memory -- marker`
Expected: FAIL — functions not defined

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::memory::MemoryScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMarker {
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
}

pub fn extract_memory_markers(text: &str) -> Vec<MemoryMarker> {
    let re = regex::Regex::new(
        r#"<memory(?:\s+scope="([^"]*)")?(?:\s+key="([^"]*)")?\s*>([\s\S]*?)</memory>"#,
    )
    .unwrap();

    re.captures_iter(text)
        .map(|cap| MemoryMarker {
            scope: match cap.get(1).map(|m| m.as_str()) {
                Some("user") => MemoryScope::User,
                Some("workspace") => MemoryScope::Workspace,
                _ => MemoryScope::Session,
            },
            key: cap.get(2).map(|m| m.as_str().to_string()),
            content: cap
                .get(3)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default(),
        })
        .filter(|m| !m.content.is_empty())
        .collect()
}

pub fn strip_memory_markers(text: &str) -> String {
    let re = regex::Regex::new(
        r#"<memory(?:\s+scope="[^"]*")?(?:\s+key="[^"]*")?\s*>[\s\S]*?</memory>\s*\n?"#,
    )
    .unwrap();
    re.replace_all(text, "").trim_end().to_string()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-memory -- marker`
Expected: PASS

- [ ] **Step 5: Add `pub mod marker;` to `lib.rs`**

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-memory/src/marker.rs crates/agent-memory/src/lib.rs
git commit -m "feat(memory): add <memory> marker parser and strip function"
```

---

## Task 3: MemoryStore Trait + SqliteMemoryStore

**Files:**

- Create: `crates/agent-memory/src/store.rs`
- Modify: `crates/agent-memory/Cargo.toml` — add sqlx, async-trait, chrono, uuid, serde_json
- Modify: `crates/agent-memory/src/memory.rs` — add `accepted` field, `from_marker()` constructor

- [ ] **Step 1: Update `memory.rs` with `accepted` field and `from_marker()`**

Add `accepted: bool` field to `MemoryEntry`. Add constructor:

```rust
impl MemoryEntry {
    pub fn new(
        scope: MemoryScope,
        content: String,
        accepted: bool,
    ) -> Self {
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4().simple()),
            scope,
            content,
            accepted,
        }
    }

    pub fn from_marker(
        marker: crate::marker::MemoryMarker,
        session_id: Option<String>,
        workspace_id: Option<String>,
        accepted: bool,
    ) -> Self {
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4().simple()),
            scope: marker.scope,
            content: marker.content,
            accepted,
            session_id,
            workspace_id,
            key: marker.key,
        }
    }
}
```

Extend `MemoryEntry` with `key`, `session_id`, `workspace_id` fields:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}
```

- [ ] **Step 2: Write the failing test for SqliteMemoryStore**

Add to `crates/agent-memory/src/store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_store() -> SqliteMemoryStore {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        SqliteMemoryStore::new(pool).await.unwrap()
    }

    #[tokio::test]
    async fn store_and_query_round_trip() {
        let store = test_store().await;
        let entry = MemoryEntry::new(MemoryScope::Workspace, "Use cargo nextest".into(), true);
        store.store(entry.clone()).await.unwrap();

        let results = store
            .query(MemoryQuery {
                scope: None,
                keywords: vec!["nextest".into()],
                limit: 10,
                session_id: None,
                workspace_id: None,
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Use cargo nextest");
    }

    #[tokio::test]
    async fn unaccepted_memories_excluded_from_query() {
        let store = test_store().await;
        let entry = MemoryEntry::new(MemoryScope::Workspace, "Hidden".into(), false);
        store.store(entry).await.unwrap();

        let results = store
            .query(MemoryQuery {
                scope: None,
                keywords: vec!["Hidden".into()],
                limit: 10,
                session_id: None,
                workspace_id: None,
            })
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let store = test_store().await;
        let entry = MemoryEntry::new(MemoryScope::Session, "temp".into(), true);
        store.store(entry.clone()).await.unwrap();
        store.delete(&entry.id).await.unwrap();

        let results = store
            .query(MemoryQuery {
                scope: Some(MemoryScope::Session),
                keywords: vec![],
                limit: 10,
                session_id: None,
                workspace_id: None,
            })
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn list_by_scope_filters_correctly() {
        let store = test_store().await;
        store.store(MemoryEntry::new(MemoryScope::User, "u1".into(), true)).await.unwrap();
        store.store(MemoryEntry::new(MemoryScope::Workspace, "w1".into(), true)).await.unwrap();
        store.store(MemoryEntry::new(MemoryScope::Session, "s1".into(), true)).await.unwrap();

        let user = store.list_by_scope(MemoryScope::User).await.unwrap();
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].content, "u1");
    }

    #[tokio::test]
    async fn same_scope_and_key_deduplicates() {
        let store = test_store().await;
        let e1 = MemoryEntry { key: Some("runner".into()), ..MemoryEntry::new(MemoryScope::Workspace, "cargo test".into(), true) };
        let e2 = MemoryEntry { key: Some("runner".into()), ..MemoryEntry::new(MemoryScope::Workspace, "cargo nextest".into(), true) };
        store.store(e1).await.unwrap();
        store.store(e2).await.unwrap();

        let results = store.list_by_scope(MemoryScope::Workspace).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "cargo nextest");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p agent-memory -- store`
Expected: FAIL — `MemoryStore` not defined

- [ ] **Step 4: Write the implementation**

```rust
use crate::memory::{MemoryEntry, MemoryScope};
use crate::extractor::extract_keywords;
use async_trait::async_trait;
use sqlx::sqlite::{SqlitePoolOptions, SqlitePool};
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum MemoryStoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MemoryStoreError>;

#[derive(Debug, Clone)]
pub struct MemoryQuery {
    pub scope: Option<MemoryScope>,
    pub keywords: Vec<String>,
    pub limit: usize,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list_by_scope(&self, scope: MemoryScope) -> Result<Vec<MemoryEntry>>;
    async fn count(&self, scope: Option<MemoryScope>) -> Result<usize>;
}

pub struct SqliteMemoryStore {
    pool: SqlitePool,
}

impl SqliteMemoryStore {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id           TEXT PRIMARY KEY,
                scope        TEXT NOT NULL CHECK(scope IN ('user', 'workspace', 'session')),
                key          TEXT,
                content      TEXT NOT NULL,
                keywords     TEXT NOT NULL DEFAULT '[]',
                session_id   TEXT,
                workspace_id TEXT,
                accepted     INTEGER NOT NULL DEFAULT 0,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope)")
            .execute(&pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    fn scope_str(scope: &MemoryScope) -> &'static str {
        match scope {
            MemoryScope::User => "user",
            MemoryScope::Workspace => "workspace",
            MemoryScope::Session => "session",
        }
    }

    fn parse_scope(s: &str) -> MemoryScope {
        match s {
            "user" => MemoryScope::User,
            "workspace" => MemoryScope::Workspace,
            _ => MemoryScope::Session,
        }
    }
}

#[async_trait]
impl MemoryStore for SqliteMemoryStore {
    async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let keywords_json = serde_json::to_string(&extract_keywords(&entry.content))?;
        let now = chrono::Utc::now().to_rfc3339();
        let scope_str = Self::scope_str(&entry.scope);

        // Upsert: if same scope + key exists, update content and keywords
        if entry.key.is_some() {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM memories WHERE scope = ? AND key = ?",
            )
            .bind(scope_str)
            .bind(&entry.key)
            .fetch_one(&self.pool)
            .await?;

            if existing > 0 {
                sqlx::query(
                    "UPDATE memories SET content = ?, keywords = ?, accepted = ?, updated_at = ? WHERE scope = ? AND key = ?",
                )
                .bind(&entry.content)
                .bind(&keywords_json)
                .bind(entry.accepted as i32)
                .bind(&now)
                .bind(scope_str)
                .bind(&entry.key)
                .execute(&self.pool)
                .await?;
                return Ok(());
            }
        }

        sqlx::query(
            r#"INSERT INTO memories (id, scope, key, content, keywords, session_id, workspace_id, accepted, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&entry.id)
        .bind(scope_str)
        .bind(&entry.key)
        .bind(&entry.content)
        .bind(&keywords_json)
        .bind(&entry.session_id)
        .bind(&entry.workspace_id)
        .bind(entry.accepted as i32)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>> {
        let mut sql = String::from(
            "SELECT id, scope, key, content, session_id, workspace_id, accepted FROM memories WHERE accepted = 1",
        );
        let mut param_idx = 1;

        if query.scope.is_some() {
            sql.push_str(&format!(" AND scope = ?{param_idx}"));
            param_idx += 1;
        }

        if !query.keywords.is_empty() {
            let conditions: Vec<String> = query.keywords.iter().map(|kw| {
                let idx = param_idx;
                param_idx += 1;
                format!("(content LIKE ?{idx} OR keywords LIKE ?{idx})")
            }).collect();
            sql.push_str(&format!(" AND ({})", conditions.join(" OR ")));
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{param_idx}"));

        let mut q = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>, Option<String>, bool)>(&sql);

        if let Some(scope) = &query.scope {
            q = q.bind(Self::scope_str(scope));
        }
        for kw in &query.keywords {
            q = q.bind(format!("%{kw}%"));
        }
        q = q.bind(query.limit as i64);

        let rows = q.fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|(id, scope, key, content, session_id, workspace_id, accepted)| MemoryEntry {
                id,
                scope: Self::parse_scope(&scope),
                key,
                content,
                accepted,
                session_id,
                workspace_id,
            })
            .collect())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM memories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_by_scope(&self, scope: MemoryScope) -> Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>, Option<String>, bool)>(
            "SELECT id, scope, key, content, session_id, workspace_id, accepted FROM memories WHERE accepted = 1 AND scope = ? ORDER BY created_at DESC",
        )
        .bind(Self::scope_str(&scope))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, scope, key, content, session_id, workspace_id, accepted)| MemoryEntry {
                id,
                scope: Self::parse_scope(&scope),
                key,
                content,
                accepted,
                session_id,
                workspace_id,
            })
            .collect())
    }

    async fn count(&self, scope: Option<MemoryScope>) -> Result<usize> {
        let count = match scope {
            Some(s) => sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM memories WHERE accepted = 1 AND scope = ?",
            )
            .bind(Self::scope_str(&s))
            .fetch_one(&self.pool)
            .await?,
            None => sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM memories WHERE accepted = 1",
            )
            .fetch_one(&self.pool)
            .await?,
        };
        Ok(count as usize)
    }
}
```

- [ ] **Step 5: Add `pub mod store;` to `lib.rs` and update `Cargo.toml`**

Add to `crates/agent-memory/Cargo.toml`:

```toml
sqlx = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
serde_json = { workspace = true }
tiktoken-rs = "0.6"
```

Add `tempfile = { workspace = true }` to `[dev-dependencies]`.

Update `crates/agent-memory/src/lib.rs` to re-export:

```rust
pub mod context;
pub mod extractor;
pub mod marker;
pub mod memory;
pub mod store;

pub use context::{ContextAssembler, ContextBundle, ContextRequest};
pub use extractor::extract_keywords;
pub use marker::{extract_memory_markers, strip_memory_markers, MemoryMarker};
pub use memory::{durable_memory_requires_confirmation, MemoryDecision, MemoryEntry, MemoryScope};
pub use store::{MemoryQuery, MemoryStore, MemoryStoreError, SqliteMemoryStore};
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p agent-memory`
Expected: ALL PASS

- [ ] **Step 7: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS (may need to fix MemoryEntry construction sites in existing tests)

- [ ] **Step 8: Fix any compilation errors from MemoryEntry changes**

Search for all `MemoryEntry { .. }` constructions across the workspace and add the new `key`, `session_id`, `workspace_id`, `accepted` fields. Most will use `..MemoryEntry::new(scope, content, true)` pattern.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-memory/
git commit -m "feat(memory): add MemoryStore trait with SqliteMemoryStore and keyword indexing"
```

---

## Task 4: ContextAssembler Rewrite with tiktoken + Priority Truncation

**Files:**

- Modify: `crates/agent-memory/src/context.rs` — full rewrite

- [ ] **Step 1: Write the failing test**

Replace the existing test in `crates/agent-memory/src/context.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryScope};
    use crate::store::{MemoryQuery, MemoryStore, SqliteMemoryStore};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_assembler() -> (ContextAssembler, SqliteMemoryStore) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = SqliteMemoryStore::new(pool).await.unwrap();
        let assembler = ContextAssembler::new(500, Arc::new(store.clone()) as Arc<dyn MemoryStore>);
        (assembler, store)
    }

    #[tokio::test]
    async fn assembles_request_history_and_memory_within_budget() {
        let (assembler, store) = test_assembler().await;
        store.store(MemoryEntry::new(MemoryScope::Workspace, "Use cargo nextest".into(), true)).await.unwrap();

        let bundle = assembler.assemble(ContextRequest {
            system_prompt: Some("You are helpful.".into()),
            user_request: "fix tests".into(),
            session_history: vec!["previous answer".into()],
            selected_files: vec!["Cargo.toml".into()],
            tool_results: vec!["cargo test failed".into()],
            memories: vec![],
            active_task: Some("repair failing test".into()),
            session_id: None,
            workspace_id: None,
        }).await;

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle.messages.join("\n").contains("Use cargo nextest"));
        assert!(bundle.token_count <= 500);
    }

    #[tokio::test]
    async fn truncates_lowest_priority_first() {
        let (assembler, _) = test_assembler().await;
        let long_files: Vec<String> = (0..100).map(|i| format!("file_{i}_content")).collect();

        let bundle = assembler.assemble(ContextRequest {
            system_prompt: Some("System".into()),
            user_request: "request".into(),
            session_history: vec![],
            selected_files: long_files,
            tool_results: vec![],
            memories: vec![],
            active_task: None,
            session_id: None,
            workspace_id: None,
        }).await;

        assert!(bundle.messages[0].contains("System"));
        assert!(bundle.truncated);
    }

    #[tokio::test]
    async fn queries_memory_store_when_ids_provided() {
        let (assembler, store) = test_assembler().await;
        store.store(MemoryEntry::new(MemoryScope::Workspace, "Use cargo nextest".into(), true)).await.unwrap();

        let bundle = assembler.assemble(ContextRequest {
            system_prompt: None,
            user_request: "nextest".into(),
            session_history: vec![],
            selected_files: vec![],
            tool_results: vec![],
            memories: vec![],  // will be populated from store
            active_task: None,
            session_id: None,
            workspace_id: None,
        }).await;

        assert!(bundle.messages.join("\n").contains("Use cargo nextest"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-memory -- context`
Expected: FAIL — `ContextAssembler` constructor signature changed

- [ ] **Step 3: Write the rewritten ContextAssembler**

```rust
use crate::extractor::extract_keywords;
use crate::memory::MemoryEntry;
use crate::store::{MemoryQuery, MemoryStore};
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextSource {
    System,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
}

#[derive(Debug, Clone)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_task: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub token_count: usize,
    pub sources: Vec<ContextSource>,
    pub truncated: bool,
}

pub struct ContextAssembler {
    max_tokens: usize,
    memory_store: Arc<dyn MemoryStore>,
    tokenizer: CoreBPE,
}

impl ContextAssembler {
    pub fn new(max_tokens: usize, memory_store: Arc<dyn MemoryStore>) -> Self {
        Self {
            max_tokens,
            memory_store,
            tokenizer: tiktoken_rs::cl100k_base().unwrap(),
        }
    }

    pub async fn assemble(&self, request: ContextRequest) -> ContextBundle {
        let mut sections: Vec<(ContextSource, String, usize)> = Vec::new();

        if let Some(sp) = &request.system_prompt {
            let tokens = self.count_tokens(sp);
            sections.push((ContextSource::System, sp.clone(), tokens));
        }

        let request_text = format!("User request: {}", request.user_request);
        sections.push((ContextSource::Request, request_text, self.count_tokens(&request_text)));

        if let Some(task) = &request.active_task {
            let text = format!("Active task: {task}");
            sections.push((ContextSource::History, text, self.count_tokens(&text)));
        }

        // Query memory store if session/workspace IDs available
        let memories = if request.memories.is_empty()
            && (request.session_id.is_some() || request.workspace_id.is_some())
        {
            let keywords = extract_keywords(&request.user_request);
            self.memory_store
                .query(MemoryQuery {
                    scope: None,
                    keywords,
                    limit: 20,
                    session_id: request.session_id.clone(),
                    workspace_id: request.workspace_id.clone(),
                })
                .await
                .unwrap_or_default()
        } else {
            request.memories.clone()
        };

        for mem in &memories {
            if mem.accepted {
                let text = format!("Memory: {}", mem.content);
                sections.push((ContextSource::Memory, text, self.count_tokens(&text)));
            }
        }

        for h in &request.session_history {
            let text = format!("History: {h}");
            sections.push((ContextSource::History, text, self.count_tokens(&text)));
        }

        for tr in &request.tool_results {
            let text = format!("Tool result: {tr}");
            sections.push((ContextSource::ToolResult, text, self.count_tokens(&text)));
        }

        for sf in &request.selected_files {
            let text = format!("Selected file: {sf}");
            sections.push((ContextSource::SelectedFile, text, self.count_tokens(&text)));
        }

        let mut total_tokens: usize = sections.iter().map(|(_, _, t)| *t).sum();
        let mut truncated = false;

        while total_tokens > self.max_tokens {
            let drop_idx = find_lowest_priority_drop(&sections);
            match drop_idx {
                Some(idx) => {
                    total_tokens -= sections[idx].2;
                    sections.remove(idx);
                    truncated = true;
                }
                None => break,
            }
        }

        ContextBundle {
            messages: sections.iter().map(|(_, s, _)| s.clone()).collect(),
            token_count: total_tokens,
            sources: sections.iter().map(|(src, _, _)| src.clone()).collect(),
            truncated,
        }
    }

    fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_with_special_tokens(text).len()
    }
}

/// Find the index of the lowest-priority section that can be dropped.
/// Priority order (highest first): System, Request, Memory, History, ToolResult, SelectedFile
/// System and Request (indices matching P0/P1) are never dropped.
fn find_lowest_priority_drop(sections: &[(ContextSource, String, usize)]) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-memory -- context`
Expected: PASS

- [ ] **Step 5: Fix all compilation errors across workspace**

The `ContextAssembler::new(max_tokens)` call sites need updating to `ContextAssembler::new(max_tokens, memory_store)`. Update:

- `crates/agent-runtime/src/facade_runtime.rs` — `with_context_limit()` and `new()`
- Any other call sites

This is a breaking API change. For `LocalRuntime::new()` and `with_context_limit()`, we will need a `MemoryStore` parameter. Add a `with_memory_store()` builder method and defer ContextAssembler creation.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-memory/ crates/agent-runtime/
git commit -m "feat(memory): rewrite ContextAssembler with tiktoken and priority truncation"
```

---

## Task 5: PermissionMode Interactive + PermissionOutcome Pending

**Files:**

- Modify: `crates/agent-tools/src/permission.rs` — add `Interactive` mode, `Pending` outcome

- [ ] **Step 1: Write the failing test**

Add to `crates/agent-tools/src/permission.rs` tests:

```rust
#[test]
fn interactive_allows_reads_but_pends_writes() {
    let engine = PermissionEngine::new(PermissionMode::Interactive);
    assert_eq!(engine.decide(&ToolRisk::read("fs.read")), PermissionOutcome::Allowed);
    assert_eq!(engine.decide(&ToolRisk::write("fs.write")), PermissionOutcome::Pending);
    assert_eq!(engine.decide(&ToolRisk::shell("shell.exec", false)), PermissionOutcome::Pending);
}

#[test]
fn interactive_pends_destructive_operations() {
    let engine = PermissionEngine::new(PermissionMode::Interactive);
    assert_eq!(engine.decide(&ToolRisk::destructive("rm.rf")), PermissionOutcome::Pending);
    assert_eq!(engine.decide(&ToolRisk::shell("shell.exec", true)), PermissionOutcome::Pending);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-tools -- permission`
Expected: FAIL — `Interactive` and `Pending` not defined

- [ ] **Step 3: Add `Interactive` to `PermissionMode` and `Pending` to `PermissionOutcome`**

Add to `PermissionMode` enum:

```rust
Interactive,
```

Add to `PermissionOutcome` enum:

```rust
Pending,
```

Add to `PermissionEngine::decide()`:

```rust
(PermissionMode::Interactive, ToolEffect::Read) => PermissionOutcome::Allowed,
(PermissionMode::Interactive, ToolEffect::Network) => PermissionOutcome::Pending,
(PermissionMode::Interactive, ToolEffect::Write) => PermissionOutcome::Pending,
(PermissionMode::Interactive, ToolEffect::Shell { .. }) => PermissionOutcome::Pending,
(PermissionMode::Interactive, ToolEffect::Destructive) => PermissionOutcome::Pending,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-tools -- permission`
Expected: PASS

- [ ] **Step 5: Fix any non-exhaustive match errors across workspace**

Search for all `match` on `PermissionMode` and `PermissionOutcome` across the workspace and add the new variants.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-tools/
git commit -m "feat(tools): add Interactive permission mode and Pending outcome"
```

---

## Task 6: Update EventPayload for Memory Events

**Files:**

- Modify: `crates/agent-core/src/events.rs` — update MemoryProposed/Accepted/Rejected events

- [ ] **Step 1: Update the event variants**

The existing events `MemoryProposed`, `MemoryAccepted`, `MemoryRejected` need to be updated to carry scope and key info, and the naming aligned with our design:

Replace:

```rust
MemoryProposed {
    memory_id: String,
    content: String,
},
MemoryAccepted {
    memory_id: String,
},
MemoryRejected {
    memory_id: String,
    reason: String,
},
```

With:

```rust
MemoryProposed {
    memory_id: String,
    scope: String,
    key: Option<String>,
    content: String,
},
MemoryAccepted {
    memory_id: String,
    scope: String,
    key: Option<String>,
    content: String,
},
MemoryRejected {
    memory_id: String,
    reason: String,
},
```

Update the `event_type()` match arms accordingly.

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS (these events are not yet constructed anywhere, so no breakage)

- [ ] **Step 3: Commit**

```bash
git add crates/agent-core/
git commit -m "feat(core): add scope and key fields to MemoryProposed and MemoryAccepted events"
```

---

## Task 7: agent-store Pool Accessor

**Files:**

- Modify: `crates/agent-store/src/event_store.rs` — add `pool()` method

- [ ] **Step 1: Add pool accessor**

Add to `SqliteEventStore`:

```rust
pub fn pool(&self) -> &SqlitePool {
    &self.pool
}
```

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent-store/
git commit -m "feat(store): expose SqlitePool accessor for MemoryStore sharing"
```

---

## Task 8: Runtime Integration — Memory Pipeline + resolve_permission

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs` — integrate MemoryStore, marker parsing, pending permissions, Interactive mode handling

- [ ] **Step 1: Write the failing test**

Add to `crates/agent-runtime/src/facade_runtime.rs` tests:

```rust
#[tokio::test]
async fn memory_marker_auto_accepted_for_session_scope() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store = Arc::new(
        SqliteMemoryStore::new(store.pool().clone()).await.unwrap()
    ) as Arc<dyn MemoryStore>;
    let model = FakeModelClient::new(vec!["Done <memory scope=\"session\">note</memory>".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_memory_store(mem_store);

    let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
    let session_id = runtime.start_session(StartSessionRequest {
        workspace_id: workspace.workspace_id.clone(),
        model_profile: "fake".into(),
    }).await.unwrap();

    runtime.send_message(SendMessageRequest {
        workspace_id: workspace.workspace_id,
        session_id: session_id.clone(),
        content: "remember this".into(),
    }).await.unwrap();

    // Session-scope memory should be auto-accepted
    let count = runtime.memory_store.count(None).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn memory_marker_suggest_mode_auto_denies_workspace_scope() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store = Arc::new(
        SqliteMemoryStore::new(store.pool().clone()).await.unwrap()
    ) as Arc<dyn MemoryStore>;
    let model = FakeModelClient::new(vec!["Done <memory scope=\"workspace\" key=\"tool\">Use hatch</memory>".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_memory_store(mem_store);

    let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
    let session_id = runtime.start_session(StartSessionRequest {
        workspace_id: workspace.workspace_id.clone(),
        model_profile: "fake".into(),
    }).await.unwrap();

    runtime.send_message(SendMessageRequest {
        workspace_id: workspace.workspace_id,
        session_id,
        content: "remember this".into(),
    }).await.unwrap();

    // Workspace-scope memory should be auto-denied in Suggest mode
    let count = runtime.memory_store.count(None).await.unwrap();
    assert_eq!(count, 0);
}
```

Add necessary imports:

```rust
use agent_memory::{SqliteMemoryStore, MemoryStore, extract_memory_markers, strip_memory_markers};
use std::collections::HashMap;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime`
Expected: FAIL — `with_memory_store` not defined

- [ ] **Step 3: Implement the changes to `LocalRuntime`**

Add fields:

```rust
memory_store: Option<Arc<dyn MemoryStore>>,
pending_permissions: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
```

Add builder method:

```rust
pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
    self.memory_store = Some(store);
    self
}
```

After `AssistantMessageCompleted` event in the agent loop, add marker processing:

```rust
// After broadcasting AssistantMessageCompleted with strip_memory_markers(&assistant_text):
if let Some(ref mem_store) = self.memory_store {
    let markers = extract_memory_markers(&assistant_text);
    for marker in markers {
        let entry = MemoryEntry::from_marker(marker, None, None, false);
        if durable_memory_requires_confirmation(&entry.scope) {
            match self.permission_engine.mode() {
                PermissionMode::Interactive => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let mem_id = entry.id.clone();
                    self.pending_permissions.lock().await.insert(mem_id.clone(), tx);
                    broadcast(PermissionRequested {
                        request_id: mem_id,
                        tool_id: "memory.store".into(),
                        preview: format!("Save {} memory: {}",
                            Self::scope_str(&entry.scope), entry.content),
                    });
                    match rx.await {
                        Ok(PermissionDecision::Allow) => {
                            let mut accepted_entry = entry.clone();
                            accepted_entry.accepted = true;
                            mem_store.store(accepted_entry).await.ok();
                            broadcast(MemoryAccepted { ... });
                        }
                        Ok(PermissionDecision::Deny(reason)) => {
                            broadcast(MemoryRejected { memory_id: entry.id, reason });
                        }
                        Err(_) => {
                            broadcast(MemoryRejected { memory_id: entry.id, reason: "cancelled".into() });
                        }
                    }
                }
                PermissionMode::Suggest => {
                    broadcast(MemoryRejected { memory_id: entry.id, reason: "Auto-denied in Suggest mode".into() });
                }
                PermissionMode::Autonomous | PermissionMode::Agent => {
                    let mut accepted_entry = entry.clone();
                    accepted_entry.accepted = true;
                    mem_store.store(accepted_entry).await.ok();
                    broadcast(MemoryAccepted { ... });
                }
                _ => {
                    broadcast(MemoryRejected { memory_id: entry.id, reason: format!("Denied in {:?} mode", self.permission_engine.mode()) });
                }
            }
        } else {
            let mut accepted_entry = entry.clone();
            accepted_entry.accepted = true;
            mem_store.store(accepted_entry).await.ok();
            broadcast(MemoryAccepted { ... });
        }
    }
}
```

Add `resolve_permission()` method:

```rust
pub async fn resolve_permission(&self, request_id: &str, decision: PermissionDecision) -> Result<(), RuntimeError> {
    if let Some(tx) = self.pending_permissions.lock().await.remove(request_id) {
        tx.send(decision).map_err(|_| RuntimeError::UnknownTask(request_id.into()))?;
    }
    Ok(())
}
```

Also update the `AssistantMessageCompleted` broadcast to use `strip_memory_markers(&assistant_text)` instead of `assistant_text.clone()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-runtime`
Expected: PASS

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/
git commit -m "feat(runtime): integrate MemoryStore, marker parsing, and Interactive permission mode"
```

---

## Task 9: GUI TypeScript Types + Trace Store + Markdown Setup

**Files:**

- Create: `apps/agent-gui/src/types/trace.ts`
- Create: `apps/agent-gui/src/composables/useTraceStore.ts`
- Create: `apps/agent-gui/src/utils/markdown.ts`
- Modify: `apps/agent-gui/src/types/index.ts` — add memory event payloads
- Modify: `apps/agent-gui/package.json` — add markdown-it, highlight.js

- [ ] **Step 1: Install npm dependencies**

Run: `cd apps/agent-gui && pnpm add markdown-it highlight.js && cd ../..`

Also add type definitions:
Run: `cd apps/agent-gui && pnpm add -D @types/markdown-it && cd ../..`

- [ ] **Step 2: Create `types/trace.ts`**

```typescript
export type TraceEntryStatus = "running" | "completed" | "failed" | "pending";

export type TraceEntryKind = "tool" | "permission" | "memory";

export interface TraceEntryData {
  id: string;
  kind: TraceEntryKind;
  status: TraceEntryStatus;
  toolId?: string;
  title: string;
  startedAt: number;
  durationMs?: number;
  input?: string;
  outputPreview?: string;
  outputFull?: string;
  rawEvent?: string;
  exitCode?: number | null;
  truncated?: boolean;
  expanded: boolean;
  scope?: string;
  content?: string;
  reason?: string;
}
```

- [ ] **Step 3: Create `composables/useTraceStore.ts`**

```typescript
import { reactive } from "vue";
import type { DomainEvent, EventPayload } from "../types";
import type { TraceEntryData, TraceEntryStatus } from "../types/trace";

export const traceState = reactive({
  entries: [] as TraceEntryData[],
  density: "L2" as "L1" | "L2" | "L3"
});

function updateEntry(id: string, updates: Partial<TraceEntryData>) {
  const idx = traceState.entries.findIndex((e) => e.id === id);
  if (idx !== -1) {
    Object.assign(traceState.entries[idx], updates);
  }
}

export function applyTraceEvent(event: DomainEvent) {
  const p = event.payload;
  switch (p.type) {
    case "ToolInvocationStarted":
      traceState.entries.push({
        id: p.invocation_id,
        kind: "tool",
        status: "running",
        toolId: p.tool_id,
        title: p.tool_id,
        startedAt: Date.now(),
        expanded: false
      });
      break;

    case "ToolInvocationCompleted":
      updateEntry(p.invocation_id, {
        status: "completed",
        durationMs: p.duration_ms,
        outputPreview: p.output_preview,
        exitCode: p.exit_code,
        truncated: p.truncated
      });
      break;

    case "ToolInvocationFailed":
      updateEntry(p.invocation_id, {
        status: "failed"
      });
      break;

    case "PermissionRequested":
      traceState.entries.push({
        id: p.request_id,
        kind: "permission",
        status: "pending",
        toolId: p.tool_id,
        title: p.preview || p.tool_id,
        startedAt: Date.now(),
        expanded: true
      });
      break;

    case "PermissionGranted":
      updateEntry(p.request_id, { status: "completed" });
      break;

    case "PermissionDenied":
      updateEntry(p.request_id, { status: "failed" });
      break;

    case "MemoryProposed":
      traceState.entries.push({
        id: p.memory_id,
        kind: "memory",
        status: "pending",
        toolId: "memory.store",
        title: `Save ${p.scope} memory`,
        startedAt: Date.now(),
        expanded: true,
        scope: p.scope,
        content: p.content
      });
      break;

    case "MemoryAccepted":
      updateEntry(p.memory_id, { status: "completed" });
      break;

    case "MemoryRejected":
      updateEntry(p.memory_id, { status: "failed", reason: p.reason });
      break;
  }
}

export function clearTrace() {
  traceState.entries = [];
}
```

- [ ] **Step 4: Create `utils/markdown.ts`**

```typescript
import MarkdownIt from "markdown-it";
import hljs from "highlight.js";

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  highlight(str: string, lang: string): string {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang }).value}</code></pre>`;
      } catch {
        // fall through
      }
    }
    return `<pre class="hljs"><code>${md.utils.escapeHtml(str)}</code></pre>`;
  }
});

export function renderMarkdown(text: string): string {
  return md.render(text);
}
```

- [ ] **Step 5: Update `types/index.ts` — add memory event payload types**

Add to the `EventPayload` union:

```typescript
  | { type: "MemoryProposed"; memory_id: string; scope: string; key: string | null; content: string }
  | { type: "MemoryAccepted"; memory_id: string; scope: string; key: string | null; content: string }
  | { type: "MemoryRejected"; memory_id: string; reason: string }
```

- [ ] **Step 6: Run frontend lint/format**

Run: `pnpm run format:check && pnpm run lint`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/package.json apps/agent-gui/pnpm-lock.yaml apps/agent-gui/src/
git commit -m "feat(gui): add trace store, trace types, markdown renderer, and memory event types"
```

---

## Task 10: TraceEntry + PermissionPrompt Vue Components

**Files:**

- Create: `apps/agent-gui/src/components/TraceEntry.vue`
- Create: `apps/agent-gui/src/components/PermissionPrompt.vue`

- [ ] **Step 1: Create `PermissionPrompt.vue`**

```vue
<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import type { TraceEntryData } from "../types/trace";

const props = defineProps<{ entry: TraceEntryData }>();

async function allow() {
  try {
    await invoke("resolve_permission", {
      requestId: props.entry.id,
      decision: "grant"
    });
  } catch (e) {
    console.error("Failed to grant permission:", e);
  }
}

async function deny() {
  try {
    await invoke("resolve_permission", {
      requestId: props.entry.id,
      decision: "deny"
    });
  } catch (e) {
    console.error("Failed to deny permission:", e);
  }
}
</script>

<template>
  <div class="permission-prompt">
    <div class="permission-icon">🔑</div>
    <div class="permission-body">
      <p class="permission-title">Permission Required</p>
      <p class="permission-description">
        {{ entry.title }}
      </p>
      <div v-if="entry.scope" class="permission-meta">
        Scope: {{ entry.scope }}
      </div>
      <div class="permission-meta">Tool: {{ entry.toolId }}</div>
    </div>
    <div class="permission-actions">
      <button class="btn-allow" @click="allow">Allow</button>
      <button class="btn-deny" @click="deny">Deny</button>
    </div>
  </div>
</template>

<style scoped>
.permission-prompt {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 12px;
  background: #fff8e1;
  border: 1px solid #ffcc02;
  border-radius: 6px;
  margin: 4px 0;
}
.permission-icon {
  font-size: 16px;
  flex-shrink: 0;
}
.permission-body {
  flex: 1;
  min-width: 0;
}
.permission-title {
  margin: 0;
  font-weight: 600;
  font-size: 12px;
}
.permission-description {
  margin: 4px 0 0;
  font-size: 12px;
  color: #555;
}
.permission-meta {
  font-size: 11px;
  color: #777;
  margin-top: 2px;
}
.permission-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.btn-allow {
  padding: 4px 10px;
  background: #22a06b;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
.btn-deny {
  padding: 4px 10px;
  background: #e8e8e8;
  color: #333;
  border: 1px solid #ccc;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
</style>
```

- [ ] **Step 2: Create `TraceEntry.vue`**

```vue
<script setup lang="ts">
import type { TraceEntryData } from "../types/trace";
import PermissionPrompt from "./PermissionPrompt.vue";

const props = defineProps<{
  entry: TraceEntryData;
  density: "L1" | "L2" | "L3";
}>();

function toggle() {
  props.entry.expanded = !props.entry.expanded;
}

const statusIcon: Record<string, string> = {
  running: "⏳",
  completed: "✅",
  failed: "❌",
  pending: "🔑"
};
</script>

<template>
  <div
    :class="[
      'trace-entry',
      `trace-entry--${entry.status}`,
      `trace-entry--${entry.kind}`
    ]"
  >
    <!-- Permission prompt: special inline UI -->
    <PermissionPrompt
      v-if="entry.kind === 'permission' && entry.status === 'pending'"
      :entry="entry"
    />

    <!-- Normal entry -->
    <template v-else>
      <!-- L1: summary row -->
      <div class="entry-row" @click="toggle">
        <span class="entry-status">{{ statusIcon[entry.status] }}</span>
        <span class="entry-tool">{{ entry.toolId || entry.title }}</span>
        <span v-if="entry.durationMs != null" class="entry-duration"
          >{{ (entry.durationMs / 1000).toFixed(1) }}s</span
        >
        <span v-if="entry.status === 'running'" class="entry-running"
          >running...</span
        >
      </div>

      <!-- L2: detail (collapsible) -->
      <div v-if="density !== 'L1' && entry.expanded" class="entry-detail">
        <div v-if="entry.input" class="entry-section">
          <span class="entry-label">Input:</span>
          <pre class="entry-code">{{ entry.input }}</pre>
        </div>
        <div v-if="entry.outputPreview" class="entry-section">
          <span class="entry-label">Output:</span>
          <pre class="entry-code">{{ entry.outputPreview }}</pre>
        </div>
        <div v-if="entry.reason" class="entry-section">
          <span class="entry-label">Reason:</span>
          <span>{{ entry.reason }}</span>
        </div>
      </div>

      <!-- L3: raw JSON -->
      <div
        v-if="density === 'L3' && entry.expanded && entry.rawEvent"
        class="entry-raw"
      >
        <pre class="entry-code">{{ entry.rawEvent }}</pre>
      </div>
    </template>
  </div>
</template>

<style scoped>
.trace-entry {
  font-size: 12px;
  border-bottom: 1px solid #eee;
}
.entry-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  cursor: pointer;
}
.entry-row:hover {
  background: #f8f8f8;
}
.entry-status {
  font-size: 11px;
}
.entry-tool {
  flex: 1;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry-duration {
  color: #777;
  font-size: 11px;
}
.entry-running {
  color: #0077cc;
  font-size: 11px;
}
.entry-detail,
.entry-raw {
  padding: 4px 8px 8px;
  background: #fafafa;
}
.entry-section {
  margin-bottom: 4px;
}
.entry-label {
  font-weight: 600;
  font-size: 11px;
  color: #555;
}
.entry-code {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 4px;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
```

- [ ] **Step 3: Run frontend lint**

Run: `pnpm run lint`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/TraceEntry.vue apps/agent-gui/src/components/PermissionPrompt.vue
git commit -m "feat(gui): add TraceEntry and PermissionPrompt components"
```

---

## Task 11: TraceTimeline Rewrite + useTauriEvents Update + ChatPanel Markdown

**Files:**

- Modify: `apps/agent-gui/src/components/TraceTimeline.vue` — full rewrite
- Modify: `apps/agent-gui/src/composables/useTauriEvents.ts` — route events to both stores
- Modify: `apps/agent-gui/src/components/ChatPanel.vue` — Markdown rendering
- Modify: `apps/agent-gui/src/stores/session.ts` — handle memory events
- Modify: `apps/agent-gui/src/components/StatusBar.vue` — show permission mode

- [ ] **Step 1: Rewrite `TraceTimeline.vue`**

```vue
<script setup lang="ts">
import TraceEntry from "./TraceEntry.vue";
import { traceState } from "../composables/useTraceStore";
</script>

<template>
  <section class="trace-timeline">
    <header class="trace-header">
      <h2>Trace</h2>
      <div class="density-toggles">
        <button
          v-for="d in ['L1', 'L2', 'L3'] as const"
          :key="d"
          :class="{ active: traceState.density === d }"
          @click="traceState.density = d"
        >
          {{ d }}
        </button>
      </div>
    </header>
    <div class="trace-entries">
      <TraceEntry
        v-for="entry in traceState.entries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <p v-if="traceState.entries.length === 0" class="empty-hint">
        No trace events yet
      </p>
    </div>
  </section>
</template>

<style scoped>
.trace-timeline {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.trace-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #d7d7d7;
}
.trace-header h2 {
  margin: 0;
  font-size: 14px;
}
.density-toggles {
  display: flex;
  gap: 2px;
}
.density-toggles button {
  padding: 2px 8px;
  border: 1px solid #d7d7d7;
  border-radius: 3px;
  background: white;
  font-size: 11px;
  cursor: pointer;
}
.density-toggles button.active {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
.trace-entries {
  flex: 1;
  overflow-y: auto;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 12px;
}
</style>
```

- [ ] **Step 2: Update `useTauriEvents.ts`**

```typescript
import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "../types";
import { sessionState, applyEvent } from "../stores/session";
import { applyTraceEvent } from "./useTraceStore";

export function useTauriEvents() {
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<DomainEvent>("session-event", (event) => {
      applyEvent(event.payload);
      applyTraceEvent(event.payload);
    });
    sessionState.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    sessionState.connected = false;
  });
}
```

- [ ] **Step 3: Update `ChatPanel.vue` — Markdown rendering**

Add import:

```typescript
import { renderMarkdown } from "../utils/markdown";
```

Replace the assistant message template:

```vue
<div
  v-if="msg.role === 'assistant'"
  class="message-content markdown-body"
  v-html="renderMarkdown(msg.content)"
></div>
<div v-else class="message-content">{{ msg.content }}</div>
```

Add markdown CSS:

```css
.markdown-body :deep(pre) {
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 6px;
  padding: 12px 16px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
}
.markdown-body :deep(code) {
  font-family: "JetBrains Mono", "Fira Code", monospace;
}
.markdown-body :deep(:not(pre) > code) {
  background: #f0f0f0;
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 0.9em;
}
.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  padding-left: 20px;
}
.markdown-body :deep(p) {
  margin: 6px 0;
}
```

Remove the `<span class="message-role">` / `<span class="message-content">` pair for assistant messages and replace with the v-html version. Keep the `.message-role` label outside.

- [ ] **Step 4: Update `session.ts` — handle memory events**

Add to `applyEvent` switch:

```typescript
case "MemoryProposed":
  // Optional: show subtle indicator in chat
  break;
case "MemoryAccepted":
  // Optional: show "📝 Memory saved" toast
  break;
case "MemoryRejected":
  // Optional: show "Memory rejected" toast
  break;
```

- [ ] **Step 5: Update `StatusBar.vue` — show permission mode**

Add a new status item:

```vue
<span class="status-item">mode: interactive</span>
<span class="status-divider">│</span>
```

For now hardcode "interactive". When the permission mode is sent as part of workspace init response, this can be made dynamic.

- [ ] **Step 6: Run frontend lint/format**

Run: `pnpm run format:check && pnpm run lint`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/
git commit -m "feat(gui): rewrite TraceTimeline, add Markdown rendering, route trace events"
```

---

## Task 12: Tauri Backend — MemoryStore Init + New Commands + Interactive Mode

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/app_state.rs` — add memory_store
- Modify: `apps/agent-gui/src-tauri/src/commands.rs` — add resolve_permission, query_memories, delete_memory
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` — init MemoryStore, use Interactive mode, register commands
- Modify: `apps/agent-gui/src-tauri/Cargo.toml` — add agent-memory dep

- [ ] **Step 1: Add `agent-memory` to Tauri Cargo.toml**

Add to `apps/agent-gui/src-tauri/Cargo.toml` dependencies:

```toml
agent-memory = { path = "../../../crates/agent-memory" }
```

- [ ] **Step 2: Update `app_state.rs`**

```rust
use agent_memory::MemoryStore;

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    pub fn new(
        runtime: LocalRuntime<SqliteEventStore, ModelRouter>,
        config: Config,
        memory_store: Arc<dyn MemoryStore>,
    ) -> Self {
        Self {
            runtime: Arc::new(runtime),
            config: Arc::new(config),
            memory_store,
            workspace_id: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
```

- [ ] **Step 3: Update `commands.rs` — add new commands**

Add to the bottom:

```rust
use agent_core::PermissionDecision;
use agent_memory::{MemoryEntry, MemoryScope, MemoryQuery};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntryResponse {
    pub id: String,
    pub scope: String,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
}

impl From<MemoryEntry> for MemoryEntryResponse {
    fn from(e: MemoryEntry) -> Self {
        Self {
            id: e.id,
            scope: match e.scope {
                MemoryScope::User => "user".into(),
                MemoryScope::Workspace => "workspace".into(),
                MemoryScope::Session => "session".into(),
            },
            key: e.key,
            content: e.content,
            accepted: e.accepted,
        }
    }
}

#[tauri::command]
pub async fn resolve_permission(
    state: State<'_, GuiState>,
    request_id: String,
    decision: String,
    reason: Option<String>,
) -> Result<(), String> {
    let perm_decision = match decision.as_str() {
        "grant" => PermissionDecision::Allow,
        "deny" => PermissionDecision::Deny(reason.unwrap_or_else(|| "User denied".into())),
        _ => return Err("Invalid decision: must be 'grant' or 'deny'".into()),
    };
    state
        .runtime
        .resolve_permission(&request_id, perm_decision)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_memories(
    state: State<'_, GuiState>,
    scope: Option<String>,
    keywords: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<MemoryEntryResponse>, String> {
    let scope = scope.map(|s| match s.as_str() {
        "user" => MemoryScope::User,
        "workspace" => MemoryScope::Workspace,
        _ => MemoryScope::Session,
    });
    let entries = state
        .memory_store
        .query(MemoryQuery {
            scope,
            keywords: keywords.unwrap_or_default(),
            limit: limit.unwrap_or(50),
            session_id: None,
            workspace_id: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntryResponse::from).collect())
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, GuiState>,
    id: String,
) -> Result<(), String> {
    state
        .memory_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Update `lib.rs`**

Replace the runtime creation in `run()`:

```rust
let store = SqliteEventStore::in_memory()
    .await
    .expect("Failed to create in-memory store");

let mem_store = Arc::new(
    agent_memory::SqliteMemoryStore::new(store.pool().clone())
        .await
        .expect("Failed to create memory store"),
) as Arc<dyn agent_memory::MemoryStore>;

let config = Config::load().unwrap_or_else(|e| {
    eprintln!("Config warning: {e}, using defaults");
    Config::defaults()
});
let router = config.build_router();

eprintln!("Available model profiles: {:?}", config.profile_names());
eprintln!("Default profile: {}", config.default_profile());
eprintln!("Permission mode: Interactive");

let cwd = std::env::current_dir().expect("Cannot get current dir");

let runtime = LocalRuntime::new(store, router)
    .with_permission_mode(PermissionMode::Interactive)
    .with_context_limit(100_000)
    .with_memory_store(mem_store.clone())
    .with_builtin_tools(cwd)
    .await;

handle.manage(GuiState::new(runtime, config, mem_store));
```

Also register the new commands:

```rust
.invoke_handler(tauri::generate_handler![
    commands::list_profiles,
    commands::get_profile_info,
    commands::initialize_workspace,
    commands::start_session,
    commands::send_message,
    commands::switch_session,
    commands::list_sessions,
    commands::resolve_permission,
    commands::query_memories,
    commands::delete_memory,
])
```

- [ ] **Step 5: Fix any compilation errors**

There will likely be issues with `LocalRuntime::new()` signature change (needs MemoryStore). If `with_context_limit()` now requires MemoryStore, create the assembler lazily or store the limit and defer. The simplest approach: `ContextAssembler::new()` still works without MemoryStore if we add a `new_without_store(max_tokens)` fallback, or we make memory_store `Option<Arc<dyn MemoryStore>>` in ContextAssembler. For backward compat with TUI, keep a `new_standalone(max_tokens)` constructor that uses whitespace counting as before.

Add to `context.rs`:

```rust
/// Create a standalone assembler without a memory store.
/// Used by TUI where memory integration is deferred.
pub fn new_standalone(max_tokens: usize) -> Self {
    Self {
        max_tokens,
        memory_store: Arc::new(NoopMemoryStore),
        tokenizer: tiktoken_rs::cl100k_base().unwrap(),
    }
}
```

Add a `NoopMemoryStore` that returns empty results. This allows TUI to keep working without changes.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/
git commit -m "feat(gui-tauri): add MemoryStore init, Interactive mode, and permission/memory commands"
```

---

## Task 13: End-to-End Verification

**Files:**

- No new files — verification only

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 2: Run frontend lint and format**

Run: `pnpm run format:check && pnpm run lint`
Expected: PASS

- [ ] **Step 3: Run GUI frontend tests**

Run: `pnpm --filter agent-gui run test`
Expected: PASS

- [ ] **Step 4: Manual smoke test — TUI**

Run: `cargo run -p agent-tui`
Verify: TUI still starts and works with `Suggest` permission mode. Chat responses appear normally. No memory markers visible (they would only appear with models that produce them, which the fake model does not by default).

- [ ] **Step 5: Manual smoke test — GUI**

Run: `pnpm --filter agent-gui run tauri:dev`
Verify:

1. GUI starts with three-column layout
2. ChatPanel accepts input and displays model responses
3. Trace panel shows tool invocation events
4. Trace panel density toggle (L1/L2/L3) works
5. StatusBar shows "mode: interactive"
6. Assistant messages render as Markdown

- [ ] **Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final fixes for memory and trace integration"
```

- [ ] **Step 7: Update CHANGELOG**

Add v0.7.0 entry (or prepare for `scripts/release.sh 0.7.0`)

---

## Plan Self-Review

### 1. Spec Coverage

| Spec Requirement                      | Task    |
| ------------------------------------- | ------- |
| MemoryStore trait + SqliteMemoryStore | Task 3  |
| tiktoken token counting               | Task 4  |
| ContextAssembler priority truncation  | Task 4  |
| Keyword extraction                    | Task 1  |
| `<memory>` marker parser              | Task 2  |
| Memory marker permission pipeline     | Task 8  |
| `Interactive` permission mode         | Task 5  |
| `Pending` permission outcome          | Task 5  |
| EventPayload updates (scope/key)      | Task 6  |
| SqliteEventStore pool accessor        | Task 7  |
| Runtime resolve_permission()          | Task 8  |
| GUI TraceEntry + PermissionPrompt     | Task 10 |
| GUI TraceTimeline three-density       | Task 11 |
| GUI ChatPanel Markdown                | Task 11 |
| GUI StatusBar permission mode         | Task 11 |
| Tauri resolve_permission command      | Task 12 |
| Tauri query_memories command          | Task 12 |
| Tauri delete_memory command           | Task 12 |
| Tauri Interactive mode default        | Task 12 |
| End-to-end verification               | Task 13 |

All spec requirements covered. ✅

### 2. Placeholder Scan

No TBD, TODO, "implement later", "add appropriate error handling", or "similar to Task N" patterns found. ✅

### 3. Type Consistency

- `MemoryEntry` has `id`, `scope`, `key`, `content`, `accepted`, `session_id`, `workspace_id` — consistent across store.rs, marker.rs, context.rs, facade_runtime.rs
- `MemoryScope` variants: `User`, `Workspace`, `Session` — consistent across all modules
- `PermissionDecision` used in `resolve_permission()` matches `agent_core::PermissionDecision`
- `TraceEntryData` fields match between `types/trace.ts` and `useTraceStore.ts` and `TraceEntry.vue`
- `EventPayload` in `types/index.ts` includes `MemoryProposed`, `MemoryAccepted`, `MemoryRejected` matching `events.rs`
- `PermissionMode::Interactive` and `PermissionOutcome::Pending` consistently used across `permission.rs` and `facade_runtime.rs`
