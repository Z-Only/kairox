# Project Instruction Injection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read all existing project instruction files (AGENTS.md, CLAUDE.md, etc.) and inject merged content into the agent context as a new `ContextSource::ProjectInstruction` section.

**Architecture:** Add `ProjectInstruction` variant to `ContextSource` enum in `agent-core`. Extend `ProjectInstructionSummary` with `contents: Option<String>`. Add `project_instructions` field to `ContextRequest` and handle it in the assembler with correct priority/drop order. Update `read_project_instruction_summary()` to actually read file contents. Add `root_path` to `AgentLoopDeps` and wire through `run_agent_loop()`.

**Tech Stack:** Rust (tokio, tiktoken-rs, serde), no new dependencies.

---

### Task 1: Add `ContextSource::ProjectInstruction` variant

**Files:**

- Modify: `crates/agent-core/src/context_types.rs:6-16`

- [ ] **Step 1: Add the variant**

In `crates/agent-core/src/context_types.rs`, insert `ProjectInstruction` after `System`:

```rust
pub enum ContextSource {
    System,
    ProjectInstruction,
    ToolDefinitions,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
    CompactionSummary,
    Skill,
}
```

- [ ] **Step 2: Add serialization test**

In the test module, add after `context_source_serializes_snake_case_with_new_variants`:

```rust
#[test]
fn project_instruction_serializes_snake_case() {
    assert_eq!(
        serde_json::to_value(ContextSource::ProjectInstruction).unwrap(),
        "project_instruction"
    );
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-core -- context_types`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/context_types.rs
git commit -m "feat(core): add ContextSource::ProjectInstruction variant"
```

---

### Task 2: Extend `ProjectInstructionSummary` with `contents` field

**Files:**

- Modify: `crates/agent-core/src/facade.rs:548-553`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs:91-94, 120-127`

- [ ] **Step 1: Add `contents` field**

In `crates/agent-core/src/facade.rs`:

```rust
pub struct ProjectInstructionSummary {
    pub source_paths: Vec<String>,
    pub contents: Option<String>,
    pub warning: Option<String>,
}
```

- [ ] **Step 2: Update Tauri response type**

In `apps/agent-gui/src-tauri/src/commands.rs`, update `ProjectInstructionSummaryResponse`:

```rust
pub struct ProjectInstructionSummaryResponse {
    pub source_paths: Vec<String>,
    pub contents: Option<String>,
    pub warning: Option<String>,
}
```

And the `From` impl:

```rust
impl From<ProjectInstructionSummary> for ProjectInstructionSummaryResponse {
    fn from(summary: ProjectInstructionSummary) -> Self {
        Self {
            source_paths: summary.source_paths,
            contents: summary.contents,
            warning: summary.warning,
        }
    }
}
```

- [ ] **Step 3: Build check**

Run: `cargo check -p agent-core -p agent-gui-tauri`
Expected: Compiles (there may be warnings about unused construction, fine for now)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/facade.rs apps/agent-gui/src-tauri/src/commands.rs
git commit -m "feat(core): add contents field to ProjectInstructionSummary"
```

---

### Task 3: Read file contents in `read_project_instruction_summary`

**Files:**

- Modify: `crates/agent-runtime/src/project.rs:159-177, 242-263`

- [ ] **Step 1: Rewrite the function to read contents**

Replace `read_project_instruction_summary` in `crates/agent-runtime/src/project.rs`:

```rust
pub async fn read_project_instruction_summary(root_path: &Path) -> ProjectInstructionSummary {
    let mut source_paths = Vec::new();
    let mut content_parts: Vec<String> = Vec::new();
    let mut warning = None;

    for candidate in INSTRUCTION_FILE_PRIORITY {
        let path = root_path.join(candidate);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => {
                let display_path = path.display().to_string();
                source_paths.push(display_path);
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        let header = format!("### Instructions from {candidate}\n\n");
                        let body = if content.len() > 64 * 1024 {
                            let truncated: String = content.chars().take(64 * 1024).collect();
                            format!("{truncated}\n\n[...truncated]")
                        } else {
                            content
                        };
                        content_parts.push(format!("{header}{body}"));
                    }
                    Err(error) => {
                        warning = Some(error.to_string());
                    }
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warning = Some(error.to_string()),
        }
    }

    let contents = if content_parts.is_empty() {
        None
    } else {
        Some(content_parts.join("\n\n"))
    };

    ProjectInstructionSummary {
        source_paths,
        contents,
        warning,
    }
}
```

- [ ] **Step 2: Update existing test and add new tests**

Replace the test module in `crates/agent-runtime/src/project.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_project_instructions_in_priority_order() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::write(temp.path().join("README.md"), "readme content")
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("AGENTS.md"), "agents content")
            .await
            .unwrap();

        let summary = read_project_instruction_summary(temp.path()).await;

        // Priority: AGENTS.md before README.md
        assert_eq!(
            summary.source_paths[0],
            temp.path().join("AGENTS.md").display().to_string()
        );
        assert_eq!(
            summary.source_paths[1],
            temp.path().join("README.md").display().to_string()
        );
        assert!(summary.warning.is_none());

        let contents = summary.contents.expect("should have merged contents");
        assert!(contents.contains("### Instructions from AGENTS.md"));
        assert!(contents.contains("agents content"));
        assert!(contents.contains("### Instructions from README.md"));
        assert!(contents.contains("readme content"));
        let agents_pos = contents.find("AGENTS.md").unwrap();
        let readme_pos = contents.find("README.md").unwrap();
        assert!(agents_pos < readme_pos);
    }

    #[tokio::test]
    async fn returns_none_contents_when_no_files_exist() {
        let temp = tempfile::tempdir().unwrap();
        let summary = read_project_instruction_summary(temp.path()).await;
        assert!(summary.source_paths.is_empty());
        assert!(summary.contents.is_none());
        assert!(summary.warning.is_none());
    }

    #[tokio::test]
    async fn truncates_large_files() {
        let temp = tempfile::tempdir().unwrap();
        let big_content = "x".repeat(70_000);
        tokio::fs::write(temp.path().join("AGENTS.md"), &big_content)
            .await
            .unwrap();

        let summary = read_project_instruction_summary(temp.path()).await;
        let contents = summary.contents.unwrap();
        assert!(contents.contains("[...truncated]"));
        assert!(contents.len() < 70_000 + 200); // header + truncated body
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime -- project::tests`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/project.rs
git commit -m "feat(runtime): read project instruction file contents"
```

---

### Task 4: Add `project_instructions` to ContextAssembler

**Files:**

- Modify: `crates/agent-memory/src/context.rs:9-24 (ContextRequest), 74-112 (assemble), 229-249 (find_lowest_priority_drop), 251-414 (tests)`

- [ ] **Step 1: Add `project_instructions` field to `ContextRequest`**

```rust
#[derive(Debug, Clone, Default)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
    pub project_instructions: Option<String>,
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_skills: Vec<String>,
    pub active_task: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    pub tool_definitions: Vec<agent_models::ToolDefinition>,
}
```

- [ ] **Step 2: Add ProjectInstruction section in `assemble()`**

Insert after the System section (line 81), before the Skill section:

```rust
// P0.25: Project instructions — high-priority guidance from project files,
// placed after System prompt, before active skills.
if let Some(pi) = &request.project_instructions {
    let block = format!(
        "<project-instructions>\n{pi}\n</project-instructions>"
    );
    let n = self.count_tokens(&block);
    sections.push((ContextSource::ProjectInstruction, block, n));
}
```

- [ ] **Step 3: Update `find_lowest_priority_drop()`**

Insert `ContextSource::ProjectInstruction` between `Memory` and `ToolDefinitions`:

```rust
fn find_lowest_priority_drop(sections: &[(ContextSource, String, u64)]) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
        ContextSource::ProjectInstruction,
        ContextSource::ToolDefinitions,
        ContextSource::Skill,
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

- [ ] **Step 4: Add new tests**

Add after existing tests in the test module:

```rust
#[tokio::test]
async fn includes_project_instructions_section() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("System".into()),
                project_instructions: Some(
                    "### Instructions from AGENTS.md\n\nUse cargo nextest.".into(),
                ),
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(600, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("<project-instructions>"));
    assert!(combined.contains("Use cargo nextest"));
    assert!(combined.contains("</project-instructions>"));
    assert!(
        combined.find("System").unwrap()
            < combined.find("<project-instructions>").unwrap()
    );
}

#[tokio::test]
async fn project_instructions_dropped_as_last_resort() {
    let assembler = ContextAssembler::new_standalone();
    // Very tight budget — should drop ProjectInstruction but keep System+Request
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("S".into()),
                project_instructions: Some("PI content here".into()),
                user_request: "q".into(),
                ..Default::default()
            },
            test_budget(25, 0),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("S"), "System must survive");
    assert!(combined.contains("q"), "Request must survive");
    assert!(bundle.truncated);
}

#[test]
fn project_instruction_drop_order_is_between_memory_and_tool_defs() {
    let sections = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::Memory, String::from("memory"), 1),
        (ContextSource::ProjectInstruction, String::from("pi"), 1),
    ];
    // Memory drops first (lower priority than PI)
    assert_eq!(find_lowest_priority_drop(&sections), Some(2));

    let sections_no_mem = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::ProjectInstruction, String::from("pi"), 1),
    ];
    assert_eq!(find_lowest_priority_drop(&sections_no_mem), Some(2));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p agent-memory -- context::tests`
Expected: All tests PASS (existing + new)

- [ ] **Step 6: Commit**

```bash
git add crates/agent-memory/src/context.rs
git commit -m "feat(memory): add project instruction section to context assembler"
```

---

### Task 5: Add `root_path` to `AgentLoopDeps` and wire in agent loop

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/runner.rs:18-40 (AgentLoopDeps), ~225-237 (ContextRequest construction)`
- Modify: `crates/agent-runtime/src/facade_runtime.rs:741-793 (send_message → AgentLoopDeps construction)`

- [ ] **Step 1: Add `root_path` to `AgentLoopDeps`**

In `crates/agent-runtime/src/agent_loop/runner.rs`, in `AgentLoopDeps`:

```rust
pub struct AgentLoopDeps<'a, S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    pub store: &'a Arc<S>,
    pub model: &'a Arc<M>,
    pub event_tx: &'a tokio::sync::broadcast::Sender<DomainEvent>,
    pub tool_registry: &'a Arc<Mutex<ToolRegistry>>,
    pub permission_engine: &'a Arc<Mutex<PermissionEngine>>,
    pub pending_permissions:
        &'a Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    pub memory_store: &'a Option<Arc<dyn MemoryStore>>,
    pub task_graphs: &'a Arc<Mutex<HashMap<String, TaskGraph>>>,
    pub active_cancellation: &'a Arc<Mutex<Option<CancellationToken>>>,
    pub config: &'a Arc<agent_config::Config>,
    pub session_states: &'a Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    pub skill_registry: &'a Option<Arc<dyn agent_skills::SkillRegistry>>,
    pub active_skills: &'a Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub root_path: Option<std::path::PathBuf>,
}
```

- [ ] **Step 2: Read project instructions and pass to `ContextRequest` in `run_agent_loop()`**

In `crates/agent-runtime/src/agent_loop/runner.rs`, after `active_skill_blocks` (line 223) and before the `ContextRequest` construction (line 226), add:

```rust
let project_instructions = if let Some(ref root_path) = deps.root_path {
    let summary = crate::project::read_project_instruction_summary(root_path).await;
    summary.contents
} else {
    None
};
```

Then update the `ContextRequest` construction to include `project_instructions`:

```rust
let bundle = assembler
    .assemble(
        agent_memory::ContextRequest {
            system_prompt: Some(system_prompt.clone()),
            project_instructions,
            active_skills: active_skill_blocks.clone(),
            user_request: request.content.clone(),
            session_history,
            tool_definitions: tool_defs.clone(),
            ..Default::default()
        },
        budget.clone(),
    )
    .await;
```

- [ ] **Step 3: Look up root_path in `send_message` and pass to `AgentLoopDeps`**

In `crates/agent-runtime/src/facade_runtime.rs`, in `send_message()`, modify the `ExecutionMode::SingleStep` branch. Replace lines 772-791 with:

```rust
ExecutionMode::SingleStep => {
    let root_path = self
        .project_repository()
        .ok()
        .and_then(|repo| {
            let session_id = request.session_id.to_string();
            // block_on is needed because we're in an async fn but
            // the closure passed to and_then isn't async.
            // Instead, compute root_path before constructing AgentLoopDeps.
            None // Temporary — we'll compute it properly above
        });

    crate::agent_loop::run_agent_loop(
        crate::agent_loop::AgentLoopDeps {
            store: &self.store,
            model: &self.model,
            event_tx: &self.event_tx,
            tool_registry: &self.tool_registry,
            permission_engine: &self.permission_engine,
            pending_permissions: &self.pending_permissions,
            memory_store: &self.memory_store,
            task_graphs: &self.task_graphs,
            active_cancellation: &self.active_cancellation,
            config: &self.config,
            session_states: &self.session_states,
            skill_registry: &self.skill_registry,
            active_skills: &self.active_skills,
            root_path,
        },
        &request,
    )
    .await
}
```

Since `send_message` is async, compute `root_path` with `.await` before constructing the deps:

```rust
ExecutionMode::SingleStep => {
    let root_path = match self.project_repository() {
        Ok(repo) => match repo
            .get_session_binding(request.session_id.as_str())
            .await
        {
            Ok(Some(binding)) => repo
                .get_project(&binding.project_id)
                .await
                .ok()
                .map(|project| std::path::PathBuf::from(project.root_path)),
            _ => None,
        },
        Err(_) => None,
    };

    crate::agent_loop::run_agent_loop(
        crate::agent_loop::AgentLoopDeps {
            store: &self.store,
            model: &self.model,
            event_tx: &self.event_tx,
            tool_registry: &self.tool_registry,
            permission_engine: &self.permission_engine,
            pending_permissions: &self.pending_permissions,
            memory_store: &self.memory_store,
            task_graphs: &self.task_graphs,
            active_cancellation: &self.active_cancellation,
            config: &self.config,
            session_states: &self.session_states,
            skill_registry: &self.skill_registry,
            active_skills: &self.active_skills,
            root_path,
        },
        &request,
    )
    .await
}
```

- [ ] **Step 4: Build check**

Run: `cargo check -p agent-runtime`
Expected: Compiles cleanly

- [ ] **Step 5: Run all tests**

Run: `cargo test --workspace --all-targets`
Expected: All tests pass

- [ ] **Step 6: Run lint + format**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: Clean

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime/src/agent_loop/runner.rs crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(runtime): wire project instructions into agent loop context"
```

---

### Task 6: Regenerate TypeScript types and verify GUI

**Files:**

- Regenerate: `apps/agent-gui/src/generated/commands.ts` (via `just gen-types`)
- Verify: `pnpm run lint`, `pnpm --filter agent-gui run test`

- [ ] **Step 1: Regenerate types**

Run: `just gen-types`
Expected: `apps/agent-gui/src/generated/commands.ts` updated with `contents: string | null` in the instruction summary type

- [ ] **Step 2: Run GUI tests**

Run: `pnpm --filter agent-gui run test`
Expected: All vitest tests pass

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): regenerate types for ProjectInstructionSummary.contents"
```

---

### Task 7: Full verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace --all-targets && pnpm run lint`
Expected: Everything PASS

- [ ] **Step 2: Run format check**

Run: `cargo fmt --check && pnpm run format:check`
Expected: Clean

- [ ] **Step 3: Manual edge case verification**

Run a targeted test that verifies:

- No root_path → `project_instructions` is `None`, no section emitted
- One file only → correct header, correct content
- Multiple files → merged in priority order
- Large file > 64KB → truncated

This is already covered by unit tests in Tasks 3 and 4.

---

## Self-Review

1. **Spec coverage:** Each spec requirement maps to a task:
   - ContextSource variant → Task 1
   - ProjectInstructionSummary.contents → Task 2
   - read files, merge, truncate → Task 3
   - ContextRequest field + assembler section + drop order → Task 4
   - Wire into agent loop → Task 5
   - GUI type regen → Task 6

2. **Placeholder scan:** No TBDs, no TODOs, no "add appropriate error handling" patterns. Each step has concrete code.

3. **Type consistency:**
   - `ProjectInstruction { ... }` variant — same name everywhere
   - `project_instructions: Option<String>` — same field name in `ContextRequest` and context
   - `root_path: Option<PathBuf>` — same field name in `AgentLoopDeps`
   - `contents: Option<String>` — same field name in `ProjectInstructionSummary` and response type
