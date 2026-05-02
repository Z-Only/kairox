# Memory Layer + GUI Trace Visualization Design

Date: 2026-05-02
Status: Draft
Scope: `crates/agent-memory`, `crates/agent-core`, `crates/agent-runtime`, `crates/agent-store`, `apps/agent-gui`
Version Target: v0.7.0

## Context

Kairox v0.6.0 has working TUI and GUI with real model adapters (OpenAI, Anthropic, Ollama), but two critical gaps limit real-world usability:

1. **Memory is a stub** — `agent-memory` is 141 lines with only type definitions and a naive `ContextAssembler` that estimates tokens via `split_whitespace().count()`. There is no persistence, no retrieval, and no integration with the agent loop.

2. **GUI trace is a placeholder** — `TraceTimeline.vue` shows "Coming soon". Users cannot see tool invocations, permission decisions, or any agent activity beyond chat messages. In addition, the GUI runs in `Suggest` permission mode which auto-denies all writes, making real tool usage impossible without switching modes.

This design addresses both gaps: upgrading Memory from stub to production layer, and building a fully interactive Trace timeline with inline permission prompts and Markdown rendering.

## Goals

1. Implement `MemoryStore` trait with SQLite persistence and keyword-based retrieval
2. Replace whitespace token counting with tiktoken-rs accurate estimation
3. Integrate memory read/write into the agent loop with prioritized context assembly
4. Add `<memory>` marker protocol for explicit agent-to-memory writes with permission gating
5. Build a three-density (L1/L2/L3) collapsible Trace timeline in the GUI
6. Implement inline permission prompts so users can approve/deny tool and memory operations
7. Add `Interactive` permission mode for GUI (contrast with TUI's `Suggest` default)
8. Render assistant messages as Markdown with syntax-highlighted code blocks
9. Strip `<memory>` markers from displayed chat output

## Non-Goals (Deferred)

- Vector/embedding-based semantic retrieval (future, requires local embedding model)
- GUI Memory Editor panel for browsing, editing, and deleting memories (future)
- Configuration hot-reload or filesystem watch
- MCP server configuration
- Multi-agent orchestration UI
- Session persistence across restarts (separate task)
- Streaming token accumulation with `requestAnimationFrame` batching (current per-event render is adequate)

## Architecture

### Data Flow

```
User message
  │
  ▼
Agent Loop
  │
  ├─ 1. ContextAssembler.assemble()
  │     ├── MemoryStore.query(keywords from user_request)
  │     ├── Prioritized message assembly: system > request > memories > history > tools > files
  │     └── tiktoken-rs token counting with budget-aware truncation
  │
  ├─ 2. ModelClient.stream(request)
  │     └── Streaming ModelEvent tokens → broadcast
  │
  ├─ 3. Marker extraction from assistant response
  │     ├── extract_memory_markers(text) → Vec<MemoryMarker>
  │     ├── strip_memory_markers(text) → clean display text
  │     └── For each marker:
  │           ├── Session scope → auto-accept, write to MemoryStore
  │           └── User/Workspace scope → PermissionEngine check
  │                 ├── Interactive mode → broadcast PermissionRequested
  │                 ├── Suggest mode → auto-deny with reason
  │                 └── Autonomous mode → auto-allow
  │
  └─ 4. Events flow to GUI via Tauri
        ├── ChatPanel: Markdown-rendered messages (markers stripped)
        ├── TraceTimeline: tool invocations, permission prompts, memory events
        └── PermissionPrompt: inline approve/deny → invoke resolve_permission
```

### Key Decisions

| Decision                 | Choice                                                  | Rationale                                                                                       |
| ------------------------ | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Token estimation         | tiktoken-rs (cl100k_base)                               | Matches GPT-4/4.1 tokenizer; accurate for context window management                             |
| Memory retrieval         | Keyword matching via SQL LIKE + keyword intersection    | No external model dependency; adequate for structured memory; extensible to vector search later |
| Memory write protocol    | Explicit `<memory>` markers in assistant output         | User-visible, permission-gatable, no extra inference step needed                                |
| Permission mode for GUI  | New `Interactive` mode                                  | Prevents auto-deny of writes; allows real tool usage through user confirmation                  |
| Trace density levels     | L1 (summary), L2 (detail), L3 (raw JSON) — matching TUI | Consistent UX across TUI and GUI                                                                |
| Markdown rendering       | markdown-it + highlight.js                              | Lightweight, well-maintained, streaming-safe                                                    |
| Memory persistence table | Dedicated `memories` table in same SQLite DB            | Reuses agent-store connection pool; no additional DB file                                       |

## Module Structure

### agent-memory (major upgrade)

```
crates/agent-memory/src/
├── lib.rs              # Public API re-exports (updated)
├── memory.rs           # MemoryEntry, MemoryScope, MemoryDecision (small changes)
├── context.rs          # ContextAssembler rewrite with tiktoken + priorities
├── store.rs            # NEW: MemoryStore trait + SqliteMemoryStore
├── marker.rs           # NEW: <memory> marker parser
└── extractor.rs        # NEW: keyword extraction from text
```

### agent-core (small changes)

Add new event variants to `events.rs`:

```rust
// In EventPayload enum
MemoryStored {
    id: String,
    scope: String,
    key: Option<String>,
    content: String,
},
MemoryRejected {
    id: String,
    reason: String,
},
```

### agent-runtime (medium changes)

- Integrate `MemoryStore` into `LocalRuntime`
- Add marker parsing step after assistant response completion
- Add pending permission request tracking for `Interactive` mode
- Add `resolve_permission()` method to `LocalRuntime`

### agent-tools (small change)

- Add `Interactive` variant to `PermissionMode` enum

### agent-store (small change)

- Add `memories` table DDL to `SqliteEventStore` initialization
- Expose SQLite pool connection for `SqliteMemoryStore`

### apps/agent-gui (major upgrade)

```
apps/agent-gui/src/
├── components/
│   ├── TraceTimeline.vue      # Rewritten: three-density timeline
│   ├── TraceEntry.vue         # NEW: single collapsible entry
│   ├── PermissionPrompt.vue   # NEW: inline approve/deny
│   ├── ChatPanel.vue          # Updated: Markdown rendering + marker stripping
│   ├── SessionsSidebar.vue    # Unchanged
│   └── StatusBar.vue          # Updated: show permission mode
├── composables/
│   ├── useTauriEvents.ts      # Updated: add trace + memory events
│   └── useTraceStore.ts       # NEW: trace state management
├── stores/
│   ├── session.ts             # Updated: handle new event types
│   └── permission.ts          # NEW: pending permission requests
├── types/
│   ├── index.ts               # Updated: new event payload types
│   └── trace.ts               # NEW: TraceEntryData types
└── utils/
    └── markdown.ts            # NEW: markdown-it setup + highlight.js

apps/agent-gui/src-tauri/src/
├── lib.rs               # Updated: Interactive permission mode, MemoryStore init
├── commands.rs          # Updated: add resolve_permission, query_memories, delete_memory
├── app_state.rs         # Updated: add MemoryStore to GuiState
└── event_forwarder.rs   # Unchanged
```

## Detailed Design

### 1. MemoryStore Trait and SqliteMemoryStore

```rust
// crates/agent-memory/src/store.rs

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    async fn query(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list_by_scope(&self, scope: MemoryScope) -> Result<Vec<MemoryEntry>>;
    async fn count(&self, scope: Option<MemoryScope>) -> Result<usize>;
}

pub struct MemoryQuery {
    pub scope: Option<MemoryScope>,
    pub keywords: Vec<String>,
    pub limit: usize,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}
```

SQLite schema:

```sql
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
);

CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
```

Key design points:

- `keywords` is a JSON array of extracted tokens, used for LIKE matching
- `accepted` column gates whether a memory appears in query results; only accepted memories are retrieved by ContextAssembler
- `key` column enables deduplication: storing with the same `scope + key` updates the existing entry instead of creating a duplicate
- `SqliteMemoryStore` takes a `sqlx::SqlitePool` reference. `SqliteEventStore` gains a `pub fn pool(&self) -> &SqlitePool` accessor so both stores share the same connection pool

### 2. Keyword Extraction

```rust
// crates/agent-memory/src/extractor.rs

/// Extract meaningful keywords from text for storage and retrieval.
/// Splits on whitespace and punctuation, filters stop words and short tokens.
pub fn extract_keywords(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| s.len() > 2)
        .filter(|s| !STOP_WORDS.contains(s))
        .take(20)
        .map(String::from)
        .collect()
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all",
    "can", "had", "her", "was", "one", "our", "out", "has",
    "this", "that", "from", "with", "have", "will", "been",
    "they", "what", "about", "which", "their", "would", "there",
];
```

Not sophisticated but sufficient. Query uses SQL:

```sql
SELECT * FROM memories
WHERE accepted = 1
  AND (scope = ? OR ? IS NULL)
  AND (
    content LIKE '%' || ?1 || '%'
    OR keywords LIKE '%' || ?1 || '%'
    OR content LIKE '%' || ?2 || '%'
    ...
  )
ORDER BY created_at DESC
LIMIT ?
```

### 3. Marker Protocol

Format:

```xml
<memory scope="workspace" key="preferred-test-runner">
  This project uses cargo nextest for testing
</memory>
```

Parser (marker.rs):

```rust
pub struct MemoryMarker {
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
}

/// Extract all <memory> markers from assistant response text.
/// Returns a list of parsed markers.
pub fn extract_memory_markers(text: &str) -> Vec<MemoryMarker> {
    let re = regex::Regex::new(
        r#"<memory(?:\s+scope="(\w+)")?(?:\s+key="([^"]+)")?\s*>([\s\S]*?)</memory>"#
    ).unwrap();
    re.captures_iter(text)
        .map(|cap| MemoryMarker {
            scope: match cap.get(1).map(|m| m.as_str()) {
                Some("user") => MemoryScope::User,
                Some("workspace") => MemoryScope::Workspace,
                _ => MemoryScope::Session,
            },
            key: cap.get(2).map(|m| m.as_str().to_string()),
            content: cap.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default(),
        })
        .filter(|m| !m.content.is_empty())
        .collect()
}

/// Remove all <memory> markers from text, producing clean display output.
pub fn strip_memory_markers(text: &str) -> String {
    let re = regex::Regex::new(
        r#"<memory(?:\s+scope="[^"]*")?(?:\s+key="[^"]*")?\s*>[\s\S]*?</memory>\s*\n?"#
    ).unwrap();
    re.replace_all(text, "").trim_end().to_string()
}
```

### 4. ContextAssembler Rewrite

```rust
// crates/agent-memory/src/context.rs

pub struct ContextAssembler {
    max_tokens: usize,
    memory_store: Arc<dyn MemoryStore>,
    tokenizer: TiktokenTokenizer,
}

pub struct ContextRequest {
    pub system_prompt: Option<String>,       // NEW: explicit system prompt
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,           // now populated by store query
    pub active_task: Option<String>,
    pub session_id: Option<String>,           // NEW: for memory scoping
    pub workspace_id: Option<String>,         // NEW: for memory scoping
}

pub struct ContextBundle {
    pub messages: Vec<String>,
    pub token_count: usize,             // accurate tiktoken count
    pub sources: Vec<ContextSource>,     // source labels for each message
    pub truncated: bool,                 // whether any content was dropped
}

pub enum ContextSource {
    System,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
}

impl ContextAssembler {
    pub fn new(max_tokens: usize, memory_store: Arc<dyn MemoryStore>) -> Self {
        Self {
            max_tokens,
            memory_store,
            tokenizer: TiktokenTokenizer::cl100k_base(),
        }
    }

    pub async fn assemble(&self, request: ContextRequest) -> ContextBundle {
        // 1. Query memory store for relevant memories
        let keywords = extract_keywords(&request.user_request);
        let memories = self.memory_store.query(MemoryQuery {
            scope: None,
            keywords,
            limit: 20,
            session_id: request.session_id.clone(),
            workspace_id: request.workspace_id.clone(),
        }).await.unwrap_or_default();

        // 2. Build prioritized message list
        //    P0 (never drop): system prompt
        //    P1 (drop last): user request
        //    P2: relevant memories
        //    P3: session history (newest first)
        //    P4: tool results
        //    P5 (drop first): selected files
        let mut sections: Vec<(ContextSource, String, usize)> = Vec::new();

        if let Some(sp) = &request.system_prompt {
            sections.push((ContextSource::System, sp.clone(), self.tiktoken_count(sp)));
        }
        sections.push((ContextSource::Request,
            format!("User request: {}", request.user_request),
            self.tiktoken_count(&request.user_request)));
        // ... assemble memories, history, tools, files with priority ordering

        // 3. Truncate from lowest priority until within budget
        let mut total_tokens: usize = sections.iter().map(|(_, _, t)| *t).sum();
        let mut truncated = false;
        while total_tokens > self.max_tokens {
            // Find lowest priority non-system, non-request section to drop
            if let Some(idx) = find_lowest_priority_drop(&sections) {
                total_tokens -= sections[idx].2;
                sections.remove(idx);
                truncated = true;
            } else {
                break;
            }
        }

        ContextBundle {
            messages: sections.into_iter().map(|(_, s, _)| s).collect(),
            token_count: total_tokens,
            sources: sections.into_iter().map(|(src, _, _)| src).collect(),
            truncated,
        }
    }

    fn tiktoken_count(&self, text: &str) -> usize {
        self.tokenizer.count(text)
    }
}
```

### 5. Runtime Integration

Changes to `LocalRuntime`:

```rust
// crates/agent-runtime/src/facade_runtime.rs

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
    memory_store: Arc<dyn MemoryStore>,    // NEW
    permission_engine: PermissionEngine,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    pending_permissions: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,  // NEW
}
```

After each assistant turn:

```rust
// In the agent loop, after ModelEvent::Complete:

let assistant_content = &accumulated_response;

// 1. Extract memory markers
let markers = extract_memory_markers(assistant_content);
let display_content = strip_memory_markers(assistant_content);

// 2. Broadcast cleaned assistant message (markers stripped)
broadcast(AssistantMessageCompleted { content: display_content, ... });

// 3. Process memory markers through permission pipeline
for marker in markers {
    let entry = MemoryEntry::from_marker(marker, session_id, workspace_id);
    if durable_memory_requires_confirmation(&entry.scope) {
        match self.permission_engine.mode() {
            PermissionMode::Interactive => {
                // Broadcast PermissionRequested, await user decision
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.pending_permissions.lock().await.insert(entry.id.clone(), tx);
                broadcast(PermissionRequested {
                    request_id: entry.id.clone(),
                    tool_id: "memory.store".into(),
                    description: format!("Save {} memory: {}", entry.scope, entry.content),
                });
                let decision = rx.await.unwrap_or(PermissionDecision::Deny);
                match decision {
                    PermissionDecision::Allow => {
                        self.memory_store.store(entry).await?;
                        broadcast(MemoryStored { ... });
                    }
                    PermissionDecision::Deny(reason) => {
                        broadcast(MemoryRejected { id: entry.id, reason });
                    }
                }
            }
            PermissionMode::Suggest => {
                broadcast(MemoryRejected { id: entry.id, reason: "Auto-denied in Suggest mode".into() });
            }
            PermissionMode::Autonomous => {
                self.memory_store.store(entry).await?;
                broadcast(MemoryStored { ... });
            }
            PermissionMode::Strict => {
                broadcast(MemoryRejected { id: entry.id, reason: "Denied in Strict mode".into() });
            }
        }
    } else {
        // Session scope: auto-accept
        self.memory_store.store(entry).await?;
        broadcast(MemoryStored { ... });
    }
}
```

New public method for GUI to resolve permissions:

```rust
impl<S, M> LocalRuntime<S, M> {
    pub async fn resolve_permission(&self, request_id: &str, decision: PermissionDecision) -> Result<()> {
        if let Some(tx) = self.pending_permissions.lock().await.remove(request_id) {
            tx.send(decision).map_err(|_| RuntimeError::UnknownTask(request_id.into()))?;
        }
        Ok(())
    }
}
```

### 6. PermissionMode Addition

```rust
// crates/agent-tools/src/permission.rs

pub enum PermissionMode {
    Autonomous,   // auto-allow all
    Suggest,      // auto-deny writes, auto-allow reads
    Strict,       // auto-deny all
    Interactive,  // request user confirmation (GUI only)
}
```

`PermissionEngine::check()` update:

- `Interactive`: return `PermissionOutcome::Pending` for write operations and memory writes
- `Pending` means the runtime will broadcast `PermissionRequested` and await `resolve_permission()`

```rust
pub enum PermissionOutcome {
    Allowed,
    Denied(String),
    Pending,   // NEW: awaiting user decision
}
```

### 7. GUI Trace Timeline

#### TraceEntry.vue

Single collapsible entry component. Renders differently per density level:

- **L1**: icon + tool_id + duration + status badge
- **L2**: L1 + collapsible input/output (syntax highlighted for code)
- **L3**: L1 + raw JSON event payload

Prop interface:

```typescript
defineProps<{
  entry: TraceEntryData;
  density: "L1" | "L2" | "L3";
}>();
```

#### TraceTimeline.vue

Container component:

```vue
<template>
  <section class="trace-timeline">
    <header class="trace-header">
      <h2>Trace</h2>
      <div class="density-toggles">
        <button
          v-for="d in ['L1', 'L2', 'L3']"
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
    </div>
  </section>
</template>
```

#### PermissionPrompt.vue

Inline approval component embedded in TraceEntry when `kind === 'permission' && status === 'pending'`:

```vue
<template>
  <div class="permission-prompt">
    <div class="permission-icon">🔑</div>
    <div class="permission-body">
      <p class="permission-title">Permission Required</p>
      <p class="permission-description">{{ entry.title }}</p>
      <div class="permission-meta">
        <span v-if="entry.toolId">Tool: {{ entry.toolId }}</span>
      </div>
    </div>
    <div class="permission-actions">
      <button class="btn-allow" @click="allow">Allow</button>
      <button class="btn-deny" @click="deny">Deny</button>
    </div>
  </div>
</template>
```

### 8. Tauri Commands

```rust
// apps/agent-gui/src-tauri/src/commands.rs additions

#[tauri::command]
pub async fn resolve_permission(
    state: State<'_, GuiState>,
    request_id: String,
    decision: String,
    reason: Option<String>,
) -> Result<(), String> {
    let decision = match decision.as_str() {
        "grant" => PermissionDecision::Allow,
        "deny" => PermissionDecision::Deny(reason.unwrap_or_else(|| "User denied".into())),
        _ => return Err("Invalid decision: must be 'grant' or 'deny'".into()),
    };
    state.runtime.resolve_permission(&request_id, decision)
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
    let entries = state.memory_store.query(MemoryQuery {
        scope,
        keywords: keywords.unwrap_or_default(),
        limit: limit.unwrap_or(50),
        session_id: None,
        workspace_id: None,
    }).await.map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(|e| MemoryEntryResponse::from(e)).collect())
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, GuiState>,
    id: String,
) -> Result<(), String> {
    state.memory_store.delete(&id)
        .await
        .map_err(|e| e.to_string())
}
```

### 9. Markdown Rendering

```typescript
// apps/agent-gui/src/utils/markdown.ts

import MarkdownIt from "markdown-it";
import hljs from "highlight.js";

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  highlight(str: string, lang: string) {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return hljs.highlight(str, { language: lang }).value;
      } catch (_) {
        /* fall through */
      }
    }
    return "";
  }
});

export function renderMarkdown(text: string): string {
  return md.render(text);
}
```

ChatPanel update:

```vue
<!-- Assistant messages: Markdown rendered -->
<div
  v-if="msg.role === 'assistant'"
  class="message-content markdown-body"
  v-html="renderMarkdown(msg.content)"
>
</div>

<!-- User messages: plain text -->
<div v-else class="message-content">{{ msg.content }}</div>
```

The `<memory>` markers are stripped before broadcasting `AssistantMessageCompleted`, so ChatPanel never sees them.

### 10. GuiState Update

```rust
// apps/agent-gui/src-tauri/src/app_state.rs

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,
    pub memory_store: Arc<dyn MemoryStore>,  // NEW
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}
```

Runtime initialization in `lib.rs`:

```rust
let pool = SqliteEventStore::create_pool(":memory:").await?;
let event_store = SqliteEventStore::new(pool.clone()).await?;
let memory_store = Arc::new(SqliteMemoryStore::new(pool).await?) as Arc<dyn MemoryStore>;
let config = Config::load().unwrap_or_else(|e| { eprintln!("Config: {e}"); Config::defaults() });
let router = config.build_router();
let runtime = LocalRuntime::new(event_store, router)
    .with_permission_mode(PermissionMode::Interactive)  // GUI default
    .with_memory_store(memory_store.clone())
    .with_builtin_tools(workspace_path).await;
```

## Testing Strategy

### agent-memory Unit Tests

- `extract_keywords`: stop words filtered, short tokens dropped, limit enforced
- `extract_memory_markers`: single marker, multiple markers, missing attributes default to session
- `strip_memory_markers`: markers removed, surrounding text preserved, multiline content handled
- `SqliteMemoryStore::store` and `query`: CRUD round-trip, keyword matching, scope filtering
- `SqliteMemoryStore::delete`: confirmed deletion
- `SqliteMemoryStore` key deduplication: same scope+key updates existing entry
- `ContextAssembler::assemble` with mock MemoryStore: prioritized assembly, token budget truncation, memory retrieval integration
- `ContextAssembler` truncation: drops P5 first, preserves P0/P1

### agent-runtime Integration Tests

- Agent loop with memory markers: session-scope marker auto-accepted
- Agent loop with memory markers: workspace-scope marker triggers PermissionRequested in Interactive mode
- `resolve_permission` grants and writes to MemoryStore
- `resolve_permission` denies and broadcasts MemoryRejected
- Suggest mode auto-denies workspace memory writes
- Autonomous mode auto-accepts workspace memory writes
- ContextAssembler includes relevant memories in next turn

### agent-tools Unit Tests

- `PermissionMode::Interactive` returns `Pending` for write operations
- `PermissionMode::Interactive` returns `Allowed` for read operations

### GUI Component Tests (Vitest)

- `useTraceStore`: correctly processes ToolInvocation* and Permission* events
- `TraceEntry.vue`: renders at L1/L2/L3 density, collapses/expands
- `PermissionPrompt.vue`: emits approve/deny events
- `ChatPanel.vue`: renders assistant messages as Markdown
- `stripMemoryMarkers`: markers not visible in rendered output
- `applyEvent`: handles MemoryStored and MemoryRejected

### Integration Test (Manual)

- Start GUI with `pnpm --filter agent-gui run tauri:dev`
- Send a message to an agent that produces `<memory>` markers
- Verify marker content is stripped from chat but appears in Trace
- Verify PermissionPrompt appears for workspace-scope memory
- Approve permission; verify memory persisted and visible in query
- Deny permission; verify MemoryRejected event in Trace
- Verify L1/L2/L3 density toggle works
- Verify Markdown rendering for code blocks, lists, bold, links
- Verify code syntax highlighting

## Acceptance Criteria

1. `agent-memory` crate has >80% test coverage for `store.rs`, `marker.rs`, `extractor.rs`, and `context.rs`
2. `ContextAssembler` uses tiktoken-rs for token counting; whitespace counting removed
3. `MemoryStore::query` returns relevant memories via keyword matching
4. `<memory>` markers parsed correctly; stripped from chat display; stored through permission pipeline
5. `Interactive` permission mode works: `PermissionRequested` broadcast → user approve/deny → MemoryStored/MemoryRejected
6. `Suggest` mode auto-denies workspace/user memory writes; `Session` scope auto-accepted in all modes
7. GUI TraceTimeline shows tool invocations at L1/L2/L3 density
8. GUI PermissionPrompt renders and resolves via `resolve_permission` Tauri command
9. GUI ChatPanel renders assistant messages as Markdown with syntax-highlighted code blocks
10. `cargo test --workspace --all-targets` passes with no regressions
11. `pnpm run format:check && pnpm run lint` passes

## Crate Dependency Changes

### agent-memory/Cargo.toml additions

```toml
[dependencies]
agent-core = { path = "../agent-core" }
sqlx = { workspace = true }
async-trait = { workspace = true }
tiktoken-rs = "0.6"
regex = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
tempfile = { workspace = true }
```

### agent-runtime/Cargo.toml additions

```toml
[dependencies]
agent-memory = { path = "../agent-memory" }  # already present, may need feature update
```

### apps/agent-gui/package.json additions

```json
{
  "dependencies": {
    "markdown-it": "^14.0.0",
    "highlight.js": "^11.10.0"
  }
}
```

## Migration Notes

- Existing `ContextAssembler::new(max_tokens)` constructor deprecated; new constructor requires `MemoryStore` parameter
- TUI will use a no-op or in-memory `MemoryStore` initially; TUI does not implement Interactive permission mode
- `PermissionMode` enum gains `Interactive` variant; existing match arms need updating
- `EventPayload` enum gains `MemoryStored` and `MemoryRejected` variants; all pattern matches need updating
- `PermissionOutcome` gains `Pending` variant; existing match arms need updating
- `GuiState` gains `memory_store` field; initialization code needs updating
