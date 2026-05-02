use crate::extractor::extract_keywords;
use crate::memory::{MemoryEntry, MemoryScope};
use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
#[cfg(test)]
use sqlx::sqlite::SqlitePoolOptions;

#[derive(Debug, thiserror::Error)]
pub enum MemoryStoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MemoryStoreError>;

#[derive(Debug, Clone)]
/// Query parameters for searching memories.
pub struct MemoryQuery {
    pub scope: Option<MemoryScope>,
    pub keywords: Vec<String>,
    pub limit: usize,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}

#[async_trait]
/// Trait for durable memory storage backends.
///
/// Implementations persist [`MemoryEntry`]s across sessions and support
/// keyword-based retrieval. The canonical implementation is [`SqliteMemoryStore`].
pub trait MemoryStore: Send + Sync {
    /// Store a new memory entry.
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    /// Query memories by scope, keywords, and limits. Only accepted memories are returned.
    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>>;
    /// Delete a memory entry by ID.
    async fn delete(&self, id: &str) -> Result<()>;
    /// List all accepted memories within a given scope.
    async fn list_by_scope(&self, scope: MemoryScope) -> Result<Vec<MemoryEntry>>;
    /// Count memories, optionally filtered by scope.
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
            let existing: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM memories WHERE scope = ? AND key = ?")
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
        let mut param_idx = 1u32;

        if query.scope.is_some() {
            sql.push_str(&format!(" AND scope = ?{param_idx}"));
            param_idx += 1;
        }

        if !query.keywords.is_empty() {
            let conditions: Vec<String> = query
                .keywords
                .iter()
                .map(|_kw| {
                    let idx = param_idx;
                    param_idx += 1;
                    format!("(content LIKE ?{idx} OR keywords LIKE ?{idx})")
                })
                .collect();
            sql.push_str(&format!(" AND ({})", conditions.join(" OR ")));
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{param_idx}"));

        let mut q = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                bool,
            ),
        >(&sql);

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
            .map(
                |(id, scope, key, content, session_id, workspace_id, accepted)| MemoryEntry {
                    id,
                    scope: Self::parse_scope(&scope),
                    key,
                    content,
                    accepted,
                    session_id,
                    workspace_id,
                },
            )
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
            .map(
                |(id, scope, key, content, session_id, workspace_id, accepted)| MemoryEntry {
                    id,
                    scope: Self::parse_scope(&scope),
                    key,
                    content,
                    accepted,
                    session_id,
                    workspace_id,
                },
            )
            .collect())
    }

    async fn count(&self, scope: Option<MemoryScope>) -> Result<usize> {
        let count = match scope {
            Some(s) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM memories WHERE accepted = 1 AND scope = ?",
                )
                .bind(Self::scope_str(&s))
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memories WHERE accepted = 1")
                    .fetch_one(&self.pool)
                    .await?
            }
        };
        Ok(count as usize)
    }
}

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
        store
            .store(MemoryEntry::new(MemoryScope::User, "u1".into(), true))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new(MemoryScope::Workspace, "w1".into(), true))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new(MemoryScope::Session, "s1".into(), true))
            .await
            .unwrap();

        let user = store.list_by_scope(MemoryScope::User).await.unwrap();
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].content, "u1");
    }

    #[tokio::test]
    async fn same_scope_and_key_deduplicates() {
        let store = test_store().await;
        let e1 = MemoryEntry {
            key: Some("runner".into()),
            ..MemoryEntry::new(MemoryScope::Workspace, "cargo test".into(), true)
        };
        let e2 = MemoryEntry {
            key: Some("runner".into()),
            ..MemoryEntry::new(MemoryScope::Workspace, "cargo nextest".into(), true)
        };
        store.store(e1).await.unwrap();
        store.store(e2).await.unwrap();

        let results = store.list_by_scope(MemoryScope::Workspace).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "cargo nextest");
    }

    #[tokio::test]
    async fn count_filters_by_scope() {
        let store = test_store().await;
        store
            .store(MemoryEntry::new(MemoryScope::User, "u1".into(), true))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new(MemoryScope::Workspace, "w1".into(), true))
            .await
            .unwrap();

        assert_eq!(store.count(None).await.unwrap(), 2);
        assert_eq!(store.count(Some(MemoryScope::User)).await.unwrap(), 1);
    }
}
