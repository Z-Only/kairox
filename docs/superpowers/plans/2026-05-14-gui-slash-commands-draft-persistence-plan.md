# GUI Slash Commands, File Mentions, and Draft Persistence — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `/` command palette, `@` file mentions, session recovery fix, and draft persistence to the kairox GUI.

**Architecture:** Backend adds SQLite draft table + session ordering fix + draft Tauri commands. Frontend adds 3 composables (CommandRegistry, MentionSearch, DraftStore) + 2 palette components (CommandPalette, FileMentionPalette) integrated into ChatPanel.vue. Recovery fix changes SQL ORDER BY + guards UI behind initialized flag + tracks last active session via localStorage.

**Tech Stack:** Rust (SQLite via sqlx, Tauri 2), Vue 3 + TypeScript, Pinia, Vitest, Playwright

---

## File Map

| File                                                    | Action     | Responsibility                                        |
| ------------------------------------------------------- | ---------- | ----------------------------------------------------- |
| `crates/agent-store/migrations/0005_session_drafts.sql` | Create     | Draft table DDL                                       |
| `crates/agent-store/src/event_store.rs`                 | Modify     | Add draft methods, fix session ordering               |
| `apps/agent-gui/src-tauri/src/commands.rs`              | Modify     | Add save_draft, get_draft Tauri commands              |
| `apps/agent-gui/src-tauri/src/lib.rs`                   | Modify     | Register new commands in generate_handler             |
| `apps/agent-gui/src-tauri/src/specta.rs`                | Modify     | Register new commands in collect_commands             |
| `apps/agent-gui/src/generated/commands.ts`              | Regenerate | New command types (via `just gen-types`)              |
| `apps/agent-gui/e2e/tauri-mock.js`                      | Modify     | Mock save_draft, get_draft                            |
| `apps/agent-gui/src/composables/useCommandRegistry.ts`  | Create     | Command definitions + matching                        |
| `apps/agent-gui/src/composables/useMentionSearch.ts`    | Create     | File fuzzy search                                     |
| `apps/agent-gui/src/composables/useDraftStore.ts`       | Create     | Draft load/save/clear                                 |
| `apps/agent-gui/src/components/CommandPalette.vue`      | Create     | `/` command popover                                   |
| `apps/agent-gui/src/components/FileMentionPalette.vue`  | Create     | `@` file mention popover                              |
| `apps/agent-gui/src/components/ChatPanel.vue`           | Modify     | Integrate palettes, draft, trigger detection          |
| `apps/agent-gui/src/stores/session.ts`                  | Modify     | Draft loading on switch, last_active_session tracking |
| `apps/agent-gui/src/App.vue`                            | Modify     | Guard rendering behind initialized flag               |

---

### Task 1: Migration for session_drafts table

**Files:**

- Create: `crates/agent-store/migrations/0005_session_drafts.sql`
- Modify: `crates/agent-store/src/event_store.rs:170-196` (add migration call)

- [ ] **Step 1: Create the migration SQL file**

```sql
CREATE TABLE IF NOT EXISTS session_drafts (
    session_id TEXT PRIMARY KEY,
    draft_text TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES kairox_sessions(session_id)
);
```

- [ ] **Step 2: Wire migration into SqliteEventStore::migrate()**

In `crates/agent-store/src/event_store.rs`, after the 0004 migration block (line 194), add:

```rust
        // 0005 adds the session_drafts table; tolerate duplicate on re-connect
        if let Err(e) = sqlx::query(include_str!(
            "../migrations/0005_session_drafts.sql"
        ))
        .execute(&self.pool)
        .await
        {
            let msg = e.to_string();
            if !msg.contains("already exists") && !msg.contains("duplicate") {
                return Err(crate::StoreError::Sqlx(e));
            }
        }
```

- [ ] **Step 3: Verify migration compiles**

Run: `cargo build -p agent-store`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/agent-store/migrations/0005_session_drafts.sql crates/agent-store/src/event_store.rs
git commit -m "feat(store): add session_drafts migration"
```

---

### Task 2: Draft methods on EventStore trait + SqliteEventStore

**Files:**

- Modify: `crates/agent-store/src/event_store.rs:13-54` (trait), after line 54 (impl)

- [ ] **Step 1: Add draft methods to EventStore trait**

In `crates/agent-store/src/event_store.rs`, add to the `EventStore` trait before the closing `}` of the trait (after line 53):

```rust
    /// Save draft text for a session (upsert).
    async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()>;
    /// Get draft text for a session, returning empty string if none exists.
    async fn get_draft(&self, session_id: &str) -> crate::Result<String>;
```

- [ ] **Step 2: Add SqliteEventStore implementation**

After the existing `cleanup_expired_sessions` implementation (search for `fn cleanup_expired_sessions` in the impl block), add:

```rust
    pub async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO session_drafts (session_id, draft_text, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET draft_text = excluded.draft_text, updated_at = excluded.updated_at",
        )
        .bind(session_id)
        .bind(draft_text)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_draft(&self, session_id: &str) -> crate::Result<String> {
        let row = sqlx::query(
            "SELECT draft_text FROM session_drafts WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r: sqlx::sqlite::SqliteRow| r.get::<String, _>("draft_text")).unwrap_or_default())
    }
```

- [ ] **Step 3: Wire into the async_trait impl block**

Find the `#[async_trait] impl EventStore for SqliteEventStore` block. Add the delegation methods:

```rust
    async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()> {
        SqliteEventStore::save_draft(self, session_id, draft_text).await
    }

    async fn get_draft(&self, session_id: &str) -> crate::Result<String> {
        SqliteEventStore::get_draft(self, session_id).await
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p agent-store`
Expected: SUCCESS

- [ ] **Step 5: Write Rust integration tests**

Add to the tests module at the bottom of `event_store.rs` (before the closing of the `#[cfg(test)]` block):

```rust
    #[tokio::test]
    async fn save_and_get_draft() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "Test".into(),
                model_profile: "fast".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .unwrap();

        // Get draft for non-existent session returns empty
        let draft = store.get_draft("ses_nonexistent").await.unwrap();
        assert_eq!(draft, "");

        // Save draft
        store.save_draft("ses_1", "hello world").await.unwrap();

        // Get draft returns saved text
        let draft = store.get_draft("ses_1").await.unwrap();
        assert_eq!(draft, "hello world");

        // Overwrite draft
        store.save_draft("ses_1", "updated").await.unwrap();
        let draft = store.get_draft("ses_1").await.unwrap();
        assert_eq!(draft, "updated");

        // Clear draft
        store.save_draft("ses_1", "").await.unwrap();
        let draft = store.get_draft("ses_1").await.unwrap();
        assert_eq!(draft, "");
    }
```

- [ ] **Step 6: Run the new test**

Run: `cargo test -p agent-store -- save_and_get_draft`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-store/src/event_store.rs
git commit -m "feat(store): add save_draft and get_draft to EventStore"
```

---

### Task 3: Fix session ordering — ORDER BY updated_at DESC

**Files:**

- Modify: `crates/agent-store/src/event_store.rs:248`

- [ ] **Step 1: Change the SQL ORDER BY**

In `crates/agent-store/src/event_store.rs`, find the `list_active_sessions` function. Change:

```sql
ORDER BY created_at ASC
```

to:

```sql
ORDER BY updated_at DESC
```

The full query becomes:

```sql
SELECT session_id, workspace_id, title, model_profile, model_id, provider, deleted_at, created_at, updated_at
FROM kairox_sessions WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY updated_at DESC
```

- [ ] **Step 2: Update the ordering test**

Find or add a test that verifies session ordering. Add to the tests module:

```rust
    #[tokio::test]
    async fn list_active_sessions_returns_most_recent_first() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now();
        let old = (now - chrono::Duration::hours(1)).to_rfc3339();
        let recent = now.to_rfc3339();

        store
            .upsert_session(&SessionRow {
                session_id: "ses_old".into(),
                workspace_id: "wrk_1".into(),
                title: "Old".into(),
                model_profile: "fast".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: old.clone(),
                updated_at: old,
            })
            .await
            .unwrap();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_recent".into(),
                workspace_id: "wrk_1".into(),
                title: "Recent".into(),
                model_profile: "fast".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: recent.clone(),
                updated_at: recent,
            })
            .await
            .unwrap();

        let sessions = store.list_active_sessions("wrk_1").await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "ses_recent");
        assert_eq!(sessions[1].session_id, "ses_old");
    }
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p agent-store -- list_active_sessions_returns_most_recent_first`
Expected: PASS

- [ ] **Step 4: Run full agent-store test suite**

Run: `cargo test -p agent-store`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-store/src/event_store.rs
git commit -m "fix(store): order sessions by updated_at DESC for correct recovery"
```

---

### Task 4: Tauri commands — save_draft and get_draft

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs` (add at end)
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` (register in generate_handler)
- Modify: `apps/agent-gui/src-tauri/src/specta.rs` (register in collect_commands)

- [ ] **Step 1: Add commands to commands.rs**

At the end of `apps/agent-gui/src-tauri/src/commands.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SaveDraftRequest {
    pub session_id: String,
    pub draft_text: String,
}

#[tauri::command]
#[specta::specta]
pub async fn save_draft(
    state: State<'_, GuiState>,
    request: SaveDraftRequest,
) -> Result<(), String> {
    state
        .runtime
        .store()
        .save_draft(&request.session_id, &request.draft_text)
        .await
        .map_err(|e| format!("Failed to save draft: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn get_draft(
    state: State<'_, GuiState>,
    session_id: String,
) -> Result<String, String> {
    state
        .runtime
        .store()
        .get_draft(&session_id)
        .await
        .map_err(|e| format!("Failed to get draft: {e}"))
}
```

Note: This requires `LocalRuntime` to expose a `store()` accessor. Check if one exists. The `LocalRuntime` wraps `store: Arc<S>`. If no public accessor exists, add one in `crates/agent-runtime/src/facade_runtime.rs`:

```rust
    pub fn store(&self) -> &S {
        &self.store
    }
```

- [ ] **Step 2: Check if store() accessor exists**

Run: `grep -n "fn store\|pub fn store" crates/agent-runtime/src/facade_runtime.rs`

If no match, add the accessor at line ~54 (after the struct fields).

- [ ] **Step 3: Register in lib.rs generate_handler!**

In `apps/agent-gui/src-tauri/src/lib.rs`, add in the `generate_handler!` macro:

```rust
            crate::commands::save_draft,
            crate::commands::get_draft,
```

- [ ] **Step 4: Register in specta.rs collect_commands!**

In `apps/agent-gui/src-tauri/src/specta.rs`, add in the `collect_commands!` macro:

```rust
            save_draft,
            get_draft,
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p agent-gui`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(gui): add save_draft and get_draft Tauri commands"
```

---

### Task 5: Regenerate TypeScript types and update mock

**Files:**

- Regenerate: `apps/agent-gui/src/generated/commands.ts`
- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Regenerate types**

Run: `just gen-types`

- [ ] **Step 2: Verify generated types**

Check that `apps/agent-gui/src/generated/commands.ts` now includes `save_draft` and `get_draft` functions.

- [ ] **Step 3: Update tauri-mock.js**

In `apps/agent-gui/e2e/tauri-mock.js`, find the handler mapping (search for `send_message` or similar) and add:

```javascript
    drafts: new Map(),

    // In the invoke handler:
    if (cmd === "save_draft") {
      state.drafts.set(args.request.session_id, args.request.draft_text);
      return;
    }
    if (cmd === "get_draft") {
      return state.drafts.get(args.session_id) || "";
    }
```

- [ ] **Step 4: Verify mock is valid JS**

Run: `node --check apps/agent-gui/e2e/tauri-mock.js`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/generated/commands.ts apps/agent-gui/e2e/tauri-mock.js
git commit -m "chore(gui): regenerate types and update mock for draft commands"
```

---

### Task 6: useCommandRegistry composable

**Files:**

- Create: `apps/agent-gui/src/composables/useCommandRegistry.ts`

- [ ] **Step 1: Write the composable**

```typescript
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useSkillsStore } from "@/stores/skills";

export interface CommandDef {
  id: string;
  label: string;
  description: string;
  /** If set, command executes immediately without inserting text */
  handler?: () => Promise<void>;
  /** If set, text inserted into input when selected (replaces the slash-trigger text) */
  insertText?: string;
  /** Context in which command is available */
  context?: "always" | "session-active";
}

export function useCommandRegistry() {
  const session = useSessionStore();
  const skills = useSkillsStore();

  const builtinCommands: CommandDef[] = [
    {
      id: "clear",
      label: "/clear",
      description: "Clear the current conversation",
      context: "session-active",
      handler: async () => {
        session.resetProjection();
      }
    },
    {
      id: "compact",
      label: "/compact",
      description: "Compact context to save tokens",
      context: "session-active",
      handler: async () => {
        if (session.currentSessionId) {
          await invoke("compact_session", { sessionId: session.currentSessionId });
        }
      }
    },
    {
      id: "model",
      label: "/model",
      description: "Switch the active model",
      context: "session-active",
      insertText: "/model "
    },
    {
      id: "help",
      label: "/help",
      description: "Show available commands",
      insertText: "/help"
    }
  ];

  const filterText = ref("");

  const matchingCommands = computed(() => {
    const q = filterText.value.toLowerCase();
    if (!q) return builtinCommands;

    return builtinCommands.filter(
      (cmd) =>
        cmd.id.toLowerCase().includes(q) ||
        cmd.label.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q)
    );
  });

  const matchingSkills = computed(() => {
    const q = filterText.value.toLowerCase();
    if (!q) return skills.activeSkills;

    return skills.activeSkills.filter(
      (s) =>
        s.skill_id.toLowerCase().includes(q) ||
        (s.display_name && s.display_name.toLowerCase().includes(q))
    );
  });

  function setFilter(text: string) {
    filterText.value = text;
  }

  function allItems() {
    const items: Array<
      | { kind: "command"; command: CommandDef }
      | { kind: "skill"; skillId: string; displayName: string }
    > = [];

    for (const cmd of matchingCommands.value) {
      if (cmd.context === "session-active" && !session.currentSessionId) continue;
      items.push({ kind: "command", command: cmd });
    }

    for (const skill of matchingSkills.value) {
      items.push({
        kind: "skill",
        skillId: skill.skill_id,
        displayName: skill.display_name || skill.skill_id
      });
    }

    return items;
  }

  return {
    filterText,
    matchingCommands,
    matchingSkills,
    setFilter,
    allItems
  };
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS (or fix type errors)

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/composables/useCommandRegistry.ts
git commit -m "feat(gui): add useCommandRegistry composable"
```

---

### Task 7: useMentionSearch composable

**Files:**

- Create: `apps/agent-gui/src/composables/useMentionSearch.ts`

- [ ] **Step 1: Write the composable**

```typescript
import { ref, shallowRef } from "vue";

export function useMentionSearch() {
  const filterText = ref("");
  const fileList = shallowRef<string[]>([]);
  const loaded = ref(false);

  async function loadFiles(workspacePath: string) {
    // Simple glob: list files in workspace, cap at 500 for performance
    // In future, replace with a Tauri command for recursive listing
    fileList.value = [];
    loaded.value = false;
    try {
      // Use the existing glob or walk approach
      // For MVP, we use a simple prefix search against a preloaded list
      // The Tauri fs API or shell tool can enumerate files
    } catch {
      // Fail silently — file list is best-effort
    }
    loaded.value = true;
  }

  function matchingFiles(): string[] {
    const q = filterText.value.toLowerCase();
    if (!q) return fileList.value.slice(0, 20);

    // Simple fuzzy: filter paths containing the query chars in order
    return fileList.value
      .filter((path) => {
        const lower = path.toLowerCase();
        let qi = 0;
        for (let i = 0; i < lower.length && qi < q.length; i++) {
          if (lower[i] === q[qi]) qi++;
        }
        return qi === q.length;
      })
      .slice(0, 20);
  }

  function setFilter(text: string) {
    filterText.value = text;
  }

  return {
    filterText,
    fileList,
    loaded,
    loadFiles,
    matchingFiles,
    setFilter
  };
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/composables/useMentionSearch.ts
git commit -m "feat(gui): add useMentionSearch composable"
```

---

### Task 8: useDraftStore composable

**Files:**

- Create: `apps/agent-gui/src/composables/useDraftStore.ts`

- [ ] **Step 1: Write the composable**

```typescript
import { invoke } from "@tauri-apps/api/core";

export function useDraftStore() {
  async function loadDraft(sessionId: string): Promise<string> {
    try {
      return await invoke<string>("get_draft", { sessionId });
    } catch {
      return "";
    }
  }

  async function saveDraft(sessionId: string, text: string): Promise<void> {
    try {
      await invoke("save_draft", {
        request: { session_id: sessionId, draft_text: text }
      });
    } catch {
      // Best-effort: silently ignore save failures
    }
  }

  async function clearDraft(sessionId: string): Promise<void> {
    await saveDraft(sessionId, "");
  }

  return { loadDraft, saveDraft, clearDraft };
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/composables/useDraftStore.ts
git commit -m "feat(gui): add useDraftStore composable"
```

---

### Task 9: CommandPalette.vue component

**Files:**

- Create: `apps/agent-gui/src/components/CommandPalette.vue`

- [ ] **Step 1: Write the component**

```vue
<script setup lang="ts">
import { useCommandRegistry, type CommandDef } from "@/composables/useCommandRegistry";

const props = defineProps<{
  visible: boolean;
  filterText: string;
}>();

const emit = defineEmits<{
  (e: "select-command", command: CommandDef): void;
  (e: "select-skill", skillId: string): void;
  (e: "close"): void;
}>();

const registry = useCommandRegistry();
registry.setFilter(props.filterText);

const items = registry.allItems();
const selectedIndex = ref(0);

watch(
  () => props.filterText,
  () => {
    registry.setFilter(props.filterText);
    selectedIndex.value = 0;
  }
);

watch(
  () => props.visible,
  (v) => {
    if (v) selectedIndex.value = 0;
  }
);

const displayedItems = computed(() => registry.allItems());

function selectItem(index: number) {
  const item = displayedItems.value[index];
  if (!item) return;
  if (item.kind === "command") {
    if (item.command.handler) {
      item.command.handler();
      emit("close");
    } else if (item.command.insertText) {
      emit("select-command", item.command);
    }
  } else if (item.kind === "skill") {
    emit("select-skill", item.skillId);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedItems.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    selectItem(selectedIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <div
    v-if="visible && displayedItems.length > 0"
    class="command-palette"
    data-test="command-palette"
    @keydown="handleKeydown"
  >
    <div class="palette-header">Commands & Skills</div>
    <div
      v-for="(item, i) in displayedItems"
      :key="item.kind === 'command' ? item.command.id : `skill-${item.skillId}`"
      class="palette-item"
      :class="{ selected: i === selectedIndex }"
      :data-test="`palette-item-${item.kind === 'command' ? item.command.id : item.skillId}`"
      @click="selectItem(i)"
      @mouseenter="selectedIndex = i"
    >
      <span class="palette-item-label">
        {{ item.kind === "command" ? item.command.label : `/skills ${item.displayName}` }}
      </span>
      <span class="palette-item-desc">
        {{ item.kind === "command" ? item.command.description : "Run skill" }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.command-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  max-height: 320px;
  overflow-y: auto;
  z-index: 100;
}
.palette-header {
  padding: 6px 12px;
  font-size: 11px;
  color: var(--color-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-bottom: 1px solid var(--color-border);
}
.palette-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
}
.palette-item.selected {
  background: var(--color-surface-hover);
}
.palette-item-label {
  font-weight: 600;
  font-size: 13px;
}
.palette-item-desc {
  font-size: 12px;
  color: var(--color-text-muted);
}
</style>
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/CommandPalette.vue
git commit -m "feat(gui): add CommandPalette component"
```

---

### Task 10: FileMentionPalette.vue component

**Files:**

- Create: `apps/agent-gui/src/components/FileMentionPalette.vue`

- [ ] **Step 1: Write the component**

```vue
<script setup lang="ts">
import { useMentionSearch } from "@/composables/useMentionSearch";
import { useSessionStore } from "@/stores/session";

const props = defineProps<{
  visible: boolean;
  filterText: string;
}>();

const emit = defineEmits<{
  (e: "select-file", path: string): void;
  (e: "close"): void;
}>();

const session = useSessionStore();
const mention = useMentionSearch();
mention.setFilter(props.filterText);

const selectedIndex = ref(0);

// Load workspace files when visible
watch(
  () => props.visible,
  (v) => {
    if (v && !mention.loaded.value) {
      // For MVP, we'd need a way to list workspace files
      // This can be enhanced later with a Tauri command
    }
    if (v) selectedIndex.value = 0;
  }
);

watch(
  () => props.filterText,
  () => {
    mention.setFilter(props.filterText);
    selectedIndex.value = 0;
  }
);

const displayedFiles = computed(() => mention.matchingFiles());

function selectFile(index: number) {
  const path = displayedFiles.value[index];
  if (path) {
    emit("select-file", path);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedFiles.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    selectFile(selectedIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <div
    v-if="visible && displayedFiles.length > 0"
    class="mention-palette"
    data-test="mention-palette"
    @keydown="handleKeydown"
  >
    <div class="palette-header">Files</div>
    <div
      v-for="(path, i) in displayedFiles"
      :key="path"
      class="palette-item"
      :class="{ selected: i === selectedIndex }"
      data-test="mention-file-item"
      @click="selectFile(i)"
      @mouseenter="selectedIndex = i"
    >
      <span class="palette-item-label">@{{ path }}</span>
    </div>
  </div>
</template>

<style scoped>
.mention-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  max-height: 320px;
  overflow-y: auto;
  z-index: 100;
}
.palette-header {
  padding: 6px 12px;
  font-size: 11px;
  color: var(--color-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-bottom: 1px solid var(--color-border);
}
.palette-item {
  padding: 8px 12px;
  cursor: pointer;
}
.palette-item.selected {
  background: var(--color-surface-hover);
}
.palette-item-label {
  font-size: 13px;
  font-family: monospace;
}
</style>
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/FileMentionPalette.vue
git commit -m "feat(gui): add FileMentionPalette component"
```

---

### Task 11: Integrate into ChatPanel.vue

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue`

- [ ] **Step 1: Add imports and composable setup**

At the top of `<script setup>` in `ChatPanel.vue`, add the new imports:

```typescript
import CommandPalette from "@/components/CommandPalette.vue";
import FileMentionPalette from "@/components/FileMentionPalette.vue";
import { useCommandRegistry, type CommandDef } from "@/composables/useCommandRegistry";
import { useDraftStore } from "@/composables/useDraftStore";
```

After the existing `const inputText = ref("");` line, add:

```typescript
// --- Palette + draft state ---
const draftStore = useDraftStore();
const commandRegistry = useCommandRegistry();
const showCommandPalette = ref(false);
const showMentionPalette = ref(false);
const paletteFilter = ref("");

// Load draft when session switches
watch(
  () => session.currentSessionId,
  async (newId, oldId) => {
    // Save draft for old session before switching
    if (oldId && inputText.value.trim()) {
      await draftStore.saveDraft(oldId, inputText.value);
    }
    // Load draft for new session
    if (newId) {
      inputText.value = await draftStore.loadDraft(newId);
    } else {
      inputText.value = "";
    }
  }
);

// Auto-save draft on input (debounced 500ms)
const debouncedSave = useDebounceFn(async (text: string) => {
  if (session.currentSessionId) {
    await draftStore.saveDraft(session.currentSessionId, text);
  }
}, 500);

watch(inputText, (val) => {
  debouncedSave(val);
});
```

Replace the existing `handleKeydown` with an `@input` handler. Add before `sendMessage`:

```typescript
// Trigger detection for / and @
function handleInput(e: Event) {
  const textarea = e.target as HTMLTextAreaElement;
  const cursorPos = textarea.selectionStart;
  const textBeforeCursor = inputText.value.slice(0, cursorPos);

  // Check for / command trigger: at start or after whitespace
  const slashMatch = textBeforeCursor.match(/^\s*\/[^\/\s]*$/);
  // Check for @ mention trigger: at start or after whitespace
  const atMatch = textBeforeCursor.match(/^\s*@[^@\s]*$/);

  if (slashMatch) {
    paletteFilter.value = textBeforeCursor.replace(/^\s*\//, "");
    showCommandPalette.value = true;
    showMentionPalette.value = false;
  } else if (atMatch) {
    paletteFilter.value = textBeforeCursor.replace(/^\s*@/, "");
    showMentionPalette.value = true;
    showCommandPalette.value = false;
  } else {
    showCommandPalette.value = false;
    showMentionPalette.value = false;
  }
}

function closePalettes() {
  showCommandPalette.value = false;
  showMentionPalette.value = false;
}

function onSelectCommand(cmd: CommandDef) {
  if (cmd.insertText) {
    // Replace the slash trigger with command text
    const cursorPos = inputText.value.length;
    const textBeforeCursor = inputText.value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/^\s*\/[^\s]*/);
    if (match) {
      const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
      const after = inputText.value.slice(cursorPos);
      inputText.value = before + cmd.insertText + after;
    }
  }
  closePalettes();
}

function onSelectSkill(skillId: string) {
  const cursorPos = inputText.value.length;
  const textBeforeCursor = inputText.value.slice(0, cursorPos);
  const match = textBeforeCursor.match(/^\s*\/[^\s]*/);
  if (match) {
    const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
    const after = inputText.value.slice(cursorPos);
    inputText.value = before + `/skills ${skillId} ` + after;
  }
  closePalettes();
}

function onSelectFile(path: string) {
  const cursorPos = inputText.value.length;
  const textBeforeCursor = inputText.value.slice(0, cursorPos);
  const match = textBeforeCursor.match(/^\s*@[^\s]*/);
  if (match) {
    const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
    const after = inputText.value.slice(cursorPos);
    inputText.value = before + `@${path} ` + after;
  }
  closePalettes();
}
```

In `sendMessage`, after the successful send, clear the draft:

```typescript
// After: inputText.value = "";
// Add:
if (session.currentSessionId) {
  draftStore.clearDraft(session.currentSessionId);
}
```

- [ ] **Step 2: Update the template — add palette components**

In the `<template>`, inside the `.input-area` div, before the `.composer-meta` div, add:

```html
<div class="palette-container">
  <CommandPalette
    :visible="showCommandPalette"
    :filter-text="paletteFilter"
    @select-command="onSelectCommand"
    @select-skill="onSelectSkill"
    @close="closePalettes"
  />
  <FileMentionPalette
    :visible="showMentionPalette"
    :filter-text="paletteFilter"
    @select-file="onSelectFile"
    @close="closePalettes"
  />
</div>
```

Add the `@input` handler to the textarea:

```html
@input="handleInput"
```

- [ ] **Step 3: Add palette-container style**

At the end of `<style scoped>`, add:

```css
.input-area {
  position: relative;
}

.palette-container {
  position: relative;
}
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue
git commit -m "feat(gui): integrate command palette, file mention, and draft persistence into ChatPanel"
```

---

### Task 12: Fix session recovery in session.ts and App.vue

**Files:**

- Modify: `apps/agent-gui/src/stores/session.ts`
- Modify: `apps/agent-gui/src/App.vue`

- [ ] **Step 1: Add last_active_session tracking in session.ts**

In `apps/agent-gui/src/stores/session.ts`, in the `switchToKnownSession` function, after `currentSessionId.value = sessionId;`, add:

```typescript
// Persist last active session for recovery
localStorage.setItem("kairox.last-active-session-id", sessionId);
```

- [ ] **Step 2: Update recoverSessions to use last_active_session**

In `recoverSessions()`, after loading `sessions.value`, modify the session selection logic. Replace:

```typescript
if (sessions.value.length > 0) {
  await switchSession(sessions.value[0].id);
}
```

with:

```typescript
if (sessions.value.length > 0) {
  // Try to restore the last active session, fall back to first (most recent)
  const lastActiveId = localStorage.getItem("kairox.last-active-session-id");
  const targetId =
    lastActiveId && sessions.value.some((s) => s.id === lastActiveId)
      ? lastActiveId
      : sessions.value[0].id;
  await switchSession(targetId);
}
```

Also update the `initialize_workspace` fallback path in App.vue (lines 31-33) with the same logic.

- [ ] **Step 3: Guard App.vue rendering behind initialized flag**

In `apps/agent-gui/src/App.vue`, change the template to show a loading state:

```html
<template>
  <AppLayout v-if="session.initialized" />
  <div v-else class="app-loading" data-test="app-loading">
    <span class="loading-spinner" />
  </div>
</template>
```

Add style:

```css
.app-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
}

.loading-spinner {
  width: 24px;
  height: 24px;
  border: 3px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `pnpm --filter agent-gui run typecheck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/session.ts apps/agent-gui/src/App.vue
git commit -m "fix(gui): restore last active session and guard UI until recovery completes"
```

---

### Task 13: Write unit tests — useCommandRegistry

**Files:**

- Create: `apps/agent-gui/src/composables/useCommandRegistry.test.ts`

- [ ] **Step 1: Write the test**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useCommandRegistry } from "./useCommandRegistry";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

// Mock stores
vi.mock("@/stores/session", () => ({
  useSessionStore: () => ({
    currentSessionId: ref("ses_1"),
    resetProjection: vi.fn()
  })
}));

vi.mock("@/stores/skills", () => ({
  useSkillsStore: () => ({
    activeSkills: [
      { skill_id: "code-review", display_name: "Code Review" },
      { skill_id: "test-gen", display_name: "Test Generator" }
    ]
  })
}));

describe("useCommandRegistry", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("returns all builtin commands when no filter", () => {
    const registry = useCommandRegistry();
    registry.setFilter("");
    const items = registry.allItems();
    expect(items.length).toBeGreaterThanOrEqual(4);
    expect(items.every((i) => i.kind === "command")).toBe(true);
  });

  it("returns skills in allItems", () => {
    const registry = useCommandRegistry();
    registry.setFilter("");
    const items = registry.allItems();
    const skillItems = items.filter((i) => i.kind === "skill");
    expect(skillItems.length).toBe(2);
  });

  it("filters commands by id", () => {
    const registry = useCommandRegistry();
    registry.setFilter("clear");
    const items = registry.allItems();
    expect(items.length).toBe(1);
    expect(items[0].kind).toBe("command");
  });

  it("filters skills by name", () => {
    const registry = useCommandRegistry();
    registry.setFilter("review");
    const items = registry.allItems();
    const skillItems = items.filter((i) => i.kind === "skill");
    expect(skillItems.length).toBe(1);
    expect(skillItems[0].skillId).toBe("code-review");
  });

  it("excludes session-only commands when no session", () => {
    const { useSessionStore } = require("@/stores/session");
    (useSessionStore as any).mockReturnValue({
      currentSessionId: null,
      resetProjection: vi.fn()
    });

    const registry = useCommandRegistry();
    registry.setFilter("");
    const items = registry.allItems();
    const clearCmd = items.find((i) => i.kind === "command" && i.command.id === "clear");
    expect(clearCmd).toBeUndefined();
  });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm --filter agent-gui run test -- useCommandRegistry`
Expected: PASS (or fix any issues)

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/composables/useCommandRegistry.test.ts
git commit -m "test(gui): add useCommandRegistry unit tests"
```

---

### Task 14: Write unit tests — useDraftStore

**Files:**

- Create: `apps/agent-gui/src/composables/useDraftStore.test.ts`

- [ ] **Step 1: Write the test**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useDraftStore } from "./useDraftStore";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: any[]) => mockInvoke(...args)
}));

describe("useDraftStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("loadDraft returns saved text", async () => {
    mockInvoke.mockResolvedValueOnce("saved draft text");
    const { loadDraft } = useDraftStore();
    const result = await loadDraft("ses_1");
    expect(result).toBe("saved draft text");
    expect(mockInvoke).toHaveBeenCalledWith("get_draft", { sessionId: "ses_1" });
  });

  it("loadDraft returns empty string on error", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
    const { loadDraft } = useDraftStore();
    const result = await loadDraft("ses_1");
    expect(result).toBe("");
  });

  it("saveDraft invokes backend command", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const { saveDraft } = useDraftStore();
    await saveDraft("ses_1", "hello");
    expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
      request: { session_id: "ses_1", draft_text: "hello" }
    });
  });

  it("clearDraft saves empty text", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const { clearDraft } = useDraftStore();
    await clearDraft("ses_1");
    expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
      request: { session_id: "ses_1", draft_text: "" }
    });
  });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm --filter agent-gui run test -- useDraftStore`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/composables/useDraftStore.test.ts
git commit -m "test(gui): add useDraftStore unit tests"
```

---

### Task 15: Write component tests — CommandPalette

**Files:**

- Create: `apps/agent-gui/src/components/CommandPalette.test.ts`

- [ ] **Step 1: Write the test**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import CommandPalette from "./CommandPalette.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/stores/session", () => ({
  useSessionStore: () => ({
    currentSessionId: ref("ses_1")
  })
}));

vi.mock("@/stores/skills", () => ({
  useSkillsStore: () => ({
    activeSkills: [{ skill_id: "hello-world", display_name: "Hello World" }]
  })
}));

describe("CommandPalette", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders when visible with items", () => {
    const wrapper = mount(CommandPalette, {
      props: { visible: true, filterText: "" }
    });
    expect(wrapper.find('[data-test="command-palette"]').exists()).toBe(true);
  });

  it("hides when not visible", () => {
    const wrapper = mount(CommandPalette, {
      props: { visible: false, filterText: "" }
    });
    expect(wrapper.find('[data-test="command-palette"]').exists()).toBe(false);
  });

  it("filters items by filterText", () => {
    const wrapper = mount(CommandPalette, {
      props: { visible: true, filterText: "clear" }
    });
    const items = wrapper.findAll(".palette-item");
    expect(items.length).toBe(1);
  });

  it("emits close on Escape", async () => {
    const wrapper = mount(CommandPalette, {
      props: { visible: true, filterText: "" }
    });
    await wrapper.trigger("keydown", { key: "Escape" });
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("emits select-command when clicking a command", async () => {
    const wrapper = mount(CommandPalette, {
      props: { visible: true, filterText: "help" }
    });
    const item = wrapper.find('[data-test="palette-item-help"]');
    await item.trigger("click");
    expect(wrapper.emitted("select-command")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm --filter agent-gui run test -- CommandPalette`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/CommandPalette.test.ts
git commit -m "test(gui): add CommandPalette component tests"
```

---

### Task 16: Run full test suite

- [ ] **Step 1: Run Rust tests**

Run: `cargo test --workspace --all-targets -- --skip lifecycle_integration`
Expected: ALL PASS

- [ ] **Step 2: Run GUI frontend tests**

Run: `pnpm --filter agent-gui run test`
Expected: ALL PASS

- [ ] **Step 3: Run lint and format check**

Run: `pnpm run lint && pnpm run format:check`
Expected: ALL PASS

- [ ] **Step 4: Commit any remaining changes**

```bash
git status
```

If clean, no commit needed.

---

### Task 17: E2E verification (manual)

- [ ] **Step 1: Build and launch GUI**

Run: `cargo build -p agent-gui`
Expected: SUCCESS

Test manually:

1.  Type `/` — verify command palette appears
2.  Type `/clear` — verify it filters to /clear, press Enter → conversation clears
3.  Type `/model` — verify text "/model " is inserted
4.  Type `/skills` — verify skills appear in palette
5.  Type `@` — verify file mention palette appears (may be empty without file list)
6.  Type a message, switch sessions, switch back — verify draft text is preserved
7.  Close app, reopen — verify last active session is restored with correct model
8.  Verify no empty-state flicker during startup

- [ ] **Step 2: Run E2E tests if available**

Run: `just test-e2e`
Expected: PASS (or fix any new failures)
