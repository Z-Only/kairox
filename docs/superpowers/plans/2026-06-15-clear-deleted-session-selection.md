# Clear Deleted Session Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the active session or active project is deleted from the GUI sidebar, the workbench must switch to a new ordinary draft conversation and clear stale chat/right-panel state.

**Architecture:** Keep deletion IPC and list mutation in the existing stores. Add sidebar-level post-delete handling because the sidebar owns the route and has access to both project/session stores. Reuse `session.startOrdinaryDraftSession()` so projection, trace, task graph, git review state, composer draft key, and persisted workbench state reset through the existing path.

**Tech Stack:** Vue 3, Pinia, Vue Router, Vitest, existing Tauri invoke mocks.

---

### Task 1: RED Tests For Sidebar Delete Cleanup

**Files:**

- Modify: `apps/agent-gui/src/composables/sidebar/useSidebarActions.test.ts`
- Modify: `apps/agent-gui/src/components/SessionsSidebar.project-sessions.test.ts`
- Modify: `apps/agent-gui/src/components/SessionsSidebar.session-actions.test.ts`
- Modify: `apps/agent-gui/src/stores/session.test.ts`
- Modify: `apps/agent-gui/src/stores/workspaceUi.test.ts`
- Test: `apps/agent-gui/src/composables/sidebar/useSidebarActions.test.ts`
- Test: `apps/agent-gui/src/components/SessionsSidebar.project-sessions.test.ts`

- [x] **Step 1: Write failing composable tests**

Add router `replace` support to the mock and assert these behaviors:

```typescript
const mockRouterReplace = vi.fn();

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: routeParams }),
  useRouter: () => ({ push: mockRouterPush, replace: mockRouterReplace })
}));
```

Add tests:

```typescript
it("starts an ordinary draft and clears the route when deleting the active ordinary session", async () => {
  routeParams.sessionId = "ses_active";
  sessionStore.currentSessionId = "ses_active";
  const actions = useSidebarActions();

  await actions.requestDeleteSession("ses_active");
  await actions.requestDeleteSession("ses_active");

  expect(sessionStore.deleteSession).toHaveBeenCalledWith("ses_active");
  expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
  expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
});

it("starts an ordinary draft and clears the route when archiving the active project session", async () => {
  routeParams.sessionId = "ps_active";
  sessionStore.currentSessionId = "ps_active";
  const actions = useSidebarActions();

  await actions.requestArchiveProjectSession("ps_active");
  await actions.requestArchiveProjectSession("ps_active");

  expect(projectStore.archiveProjectSession).toHaveBeenCalledWith("ps_active");
  expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
  expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
});

it("starts an ordinary draft and clears the route when deleting the active project", async () => {
  routeParams.sessionId = "ps_active";
  sessionStore.currentSessionId = "ps_active";
  sessionStore.currentSessionInfo = { project_id: "proj_del" };
  const actions = useSidebarActions();

  await actions.requestDeleteProject("proj_del");
  await actions.requestDeleteProject("proj_del");

  expect(projectStore.removeProject).toHaveBeenCalledWith("proj_del");
  expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
  expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
});
```

- [x] **Step 2: Write failing component integration tests**

In `SessionsSidebar.project-sessions.test.ts`, seed an active project/session with stale projection and trace, trigger the delete buttons twice, and assert:

```typescript
expect(sessionStore.currentSessionId).toBeNull();
expect(sessionStore.composerDraftKey).toBe("new-session:ordinary");
expect(sessionStore.projection.messages).toEqual([]);
expect(traceState.entries).toEqual([]);
expect(router.currentRoute.value.params.sessionId).toBeUndefined();
```

- [x] **Step 3: Run RED tests**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/composables/sidebar/useSidebarActions.test.ts src/components/SessionsSidebar.project-sessions.test.ts
```

Expected: FAIL because delete handlers do not call `startOrdinaryDraftSession()` or clear the route after successful delete.

### Task 2: GREEN Implementation

**Files:**

- Modify: `apps/agent-gui/src/composables/sidebar/useSidebarActions.ts`
- Modify: `apps/agent-gui/src/stores/session.ts`
- Modify: `apps/agent-gui/src/stores/workspaceUi.ts`

- [x] **Step 1: Add minimal helper**

Add:

```typescript
async function switchToOrdinaryDraftIfActive(wasActive: boolean) {
  if (!wasActive) return;
  await session.startOrdinaryDraftSession();
  await router.replace({ name: "workbench" });
}
```

- [x] **Step 2: Use helper after successful deletes**

Update:

```typescript
async function requestDeleteSession(sessionId: string) {
  if (pendingDeleteSessionId.value !== sessionId) {
    pendingDeleteSessionId.value = sessionId;
    pendingDeleteProjectId.value = null;
    return;
  }
  const wasActive = activeSessionId.value === sessionId || session.currentSessionId === sessionId;
  await session.deleteSession(sessionId);
  await switchToOrdinaryDraftIfActive(wasActive);
  pendingDeleteSessionId.value = null;
}
```

Update project session archive similarly:

```typescript
const wasActive = activeSessionId.value === sessionId || session.currentSessionId === sessionId;
await projects.archiveProjectSession(sessionId);
await switchToOrdinaryDraftIfActive(wasActive);
```

Update project delete similarly:

```typescript
const wasActiveProject = session.currentSessionInfo?.project_id === projectId;
await projects.removeProject(projectId);
await switchToOrdinaryDraftIfActive(wasActiveProject);
```

- [x] **Step 3: Clear git review through draft reset**

Add a `workspaceUi.clearGitReview()` reset and call it from the shared session draft reset path, so ordinary/project drafts and deletion-triggered draft switches do not preserve stale Changes panel data.

- [x] **Step 4: Run GREEN tests**

Run the focused Vitest command. Result: PASS.

### Task 3: Quality Gates And Dev App Verification

**Files:**

- No additional source files expected.

- [x] **Step 1: Run GUI checks**

Run:

```bash
bun run format:check
bun run lint
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/composables/sidebar/useSidebarActions.test.ts src/components/SessionsSidebar.project-sessions.test.ts src/stores/session-ipc.test.ts
```

- [x] **Step 2: Dev App verification**

Run:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Use `tauri-pilot` to create/open a session or project session, delete it from the sidebar, and assert the workbench has no stale session route, chat messages, trace entries, task entries, or trajectory session id.

Result: with `HOME=/tmp/kairox-delete-selection-home`, verified active ordinary session deletion returns to `#/workbench` with empty chat and empty trace; verified active project deletion after loading Changes returns to `#/workbench`, removes the project, shows empty chat/trace, and leaves Changes in the empty `Open repository changes from a project chat.` state with no console errors.

- [x] **Step 3: Prepare PR evidence**

Record commands, focused test output, and Dev App result in the PR body before push/create PR.
