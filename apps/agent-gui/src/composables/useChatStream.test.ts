import { describe, it, expect } from "vitest";
import type { CompactionStatus } from "@/types";
import type { TraceEntryData, TraceEntryKind } from "@/types/trace";
import { buildChatStream, type ChatStreamMessageInput } from "./useChatStream";
import type {
  ChatCompactionStreamItem,
  ChatMessageStreamItem,
  ChatMonitorStreamItem,
  ChatPermissionGroupStreamItem,
  ChatPermissionStreamItem,
  ChatToolCallStreamItem
} from "@/types/chatStream";

const idle: CompactionStatus = { type: "Idle" };

function toolEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "tool",
    status: "completed",
    title: "tool",
    startedAt: 0,
    expanded: false,
    ...overrides
  };
}

function permissionEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "permission",
    status: "pending",
    title: "perm",
    startedAt: 0,
    expanded: true,
    ...overrides
  };
}

function memoryEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "memory",
    status: "pending",
    title: "mem",
    startedAt: 0,
    expanded: true,
    ...overrides
  };
}

describe("buildChatStream", () => {
  it("returns an empty list when there are no messages, no trace entries, and compaction is Idle", () => {
    const result = buildChatStream([], [], idle);
    expect(result).toEqual([]);
  });

  it("emits one ChatMessageStreamItem per message in projection order with stable msg-<index> ids", () => {
    const messages: ChatStreamMessageInput[] = [
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi there", sourceAgentId: "worker-1" }
    ];

    const result = buildChatStream(messages, [], idle);

    expect(result).toHaveLength(2);
    expect(result[0]).toEqual<ChatMessageStreamItem>({
      kind: "message",
      id: "msg-0",
      role: "user",
      content: "hello"
    });
    expect(result[1]).toEqual<ChatMessageStreamItem>({
      kind: "message",
      id: "msg-1",
      role: "assistant",
      content: "hi there",
      sourceAgentId: "worker-1"
    });
  });

  it("maps tool / permission / memory trace entries to their typed stream items", () => {
    // Interleave a second tool call between the permission and memory
    // entries so they are NOT consecutive — this test asserts the
    // per-variant mapping, not the grouping behavior (see the
    // "consecutive pending permission grouping" describe block).
    const entries: TraceEntryData[] = [
      toolEntry({
        id: "tool-1",
        toolId: "shell",
        title: "shell exec",
        status: "running",
        durationMs: 42,
        input: "echo hi",
        outputPreview: "hi",
        scope: "session",
        startedAt: 1
      }),
      permissionEntry({
        id: "perm-1",
        toolId: "fs.write",
        title: "Allow write?",
        input: "/tmp/x",
        scope: "session",
        rawEvent: '{"type":"PermissionRequested"}',
        startedAt: 2
      }),
      toolEntry({
        id: "tool-2",
        toolId: "shell",
        title: "shell exec 2",
        status: "completed",
        startedAt: 3
      }),
      memoryEntry({
        id: "mem-1",
        scope: "workspace",
        content: "remember this",
        reason: "user request",
        title: "Save workspace memory",
        startedAt: 4
      })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result).toHaveLength(4);

    expect(result[0]).toEqual<ChatToolCallStreamItem>({
      kind: "tool_call",
      id: "tool-1",
      toolId: "shell",
      title: "shell exec",
      status: "running",
      durationMs: 42,
      startedAt: 1,
      input: "echo hi",
      outputPreview: "hi",
      scope: "session"
    });

    expect(result[1]).toEqual<ChatPermissionStreamItem>({
      kind: "permission",
      id: "perm-1",
      variant: "tool",
      toolId: "fs.write",
      title: "Allow write?",
      input: "/tmp/x",
      scope: "session",
      rawEvent: '{"type":"PermissionRequested"}'
    });

    expect(result[2]).toEqual<ChatToolCallStreamItem>({
      kind: "tool_call",
      id: "tool-2",
      toolId: "shell",
      title: "shell exec 2",
      status: "completed",
      startedAt: 3
    });

    expect(result[3]).toEqual<ChatPermissionStreamItem>({
      kind: "permission",
      id: "mem-1",
      variant: "memory",
      title: "Save workspace memory",
      scope: "workspace",
      content: "remember this",
      reason: "user request"
    });
  });

  it("skips trace entries whose kind is not tool / permission / memory", () => {
    const unknownEntry: TraceEntryData = {
      id: "unk-1",
      // Force an unknown kind for the defensive default branch.
      kind: "unknown" as unknown as TraceEntryKind,
      status: "completed",
      title: "?",
      startedAt: 0,
      expanded: false
    };

    const result = buildChatStream([], [unknownEntry], idle);
    expect(result).toEqual([]);
  });

  it("does not append a compaction item when compaction.type === 'Idle'", () => {
    const result = buildChatStream([{ role: "user", content: "x" }], [], idle);
    expect(result.some((item) => item.kind === "compaction")).toBe(false);
  });

  it("appends exactly one ChatCompactionStreamItem at the end when compaction.type === 'Running'", () => {
    const running: CompactionStatus = { type: "Running" };
    const messages: ChatStreamMessageInput[] = [{ role: "user", content: "hello" }];
    const entries: TraceEntryData[] = [toolEntry({ id: "tool-1" })];

    const result = buildChatStream(messages, entries, running);

    expect(result).toHaveLength(3);
    const compactionItem = result[result.length - 1];
    expect(compactionItem).toEqual<ChatCompactionStreamItem>({
      kind: "compaction",
      id: "compaction-Running",
      status: running
    });
    expect(result.filter((item) => item.kind === "compaction")).toHaveLength(1);
  });

  it("appends a Skipped compaction item at the end with id 'compaction-Skipped'", () => {
    const skipped: CompactionStatus = {
      type: "Skipped",
      reason: { type: "AlreadyCompacting" },
      ratio: 0.5
    };
    const messages: ChatStreamMessageInput[] = [{ role: "user", content: "hi" }];

    const result = buildChatStream(messages, [], skipped);

    expect(result).toHaveLength(2);
    const last = result[result.length - 1];
    expect(last).toEqual<ChatCompactionStreamItem>({
      kind: "compaction",
      id: "compaction-Skipped",
      status: skipped
    });
  });

  it("preserves trace insertion order when entries share equal startedAt values", () => {
    const entries: TraceEntryData[] = [
      toolEntry({ id: "first", title: "first", startedAt: 0 }),
      toolEntry({ id: "second", title: "second", startedAt: 0 }),
      toolEntry({ id: "third", title: "third", startedAt: 0 })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result.map((item) => item.id)).toEqual(["first", "second", "third"]);
  });

  it("orders trace items by startedAt ascending when timestamps differ", () => {
    const entries: TraceEntryData[] = [
      toolEntry({ id: "c", startedAt: 30 }),
      toolEntry({ id: "a", startedAt: 10 }),
      toolEntry({ id: "b", startedAt: 20 })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result.map((item) => item.id)).toEqual(["a", "b", "c"]);
  });

  describe("consecutive pending permission grouping", () => {
    it("collapses 3 consecutive pending permissions with different tool ids into a single permission_group", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-1", toolId: "shell", startedAt: 10 }),
        permissionEntry({ id: "perm-2", toolId: "fs.write", startedAt: 11 }),
        permissionEntry({ id: "perm-3", toolId: "patch", startedAt: 12 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      expect(result[0]).toEqual<ChatPermissionGroupStreamItem>({
        kind: "permission_group",
        id: "permission-group-perm-1",
        startedAt: 10,
        toolIds: ["shell", "fs.write", "patch"],
        permissionIds: ["perm-1", "perm-2", "perm-3"],
        count: 3
      });
    });

    it("deduplicates tool ids when consecutive pending permissions share the same tool id", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-1", toolId: "shell", startedAt: 5 }),
        permissionEntry({ id: "perm-2", toolId: "shell", startedAt: 6 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      const group = result[0] as ChatPermissionGroupStreamItem;
      expect(group.kind).toBe("permission_group");
      expect(group.count).toBe(2);
      expect(group.toolIds).toEqual(["shell"]);
      expect(group.permissionIds).toEqual(["perm-1", "perm-2"]);
      expect(group.startedAt).toBe(5);
      expect(group.id).toBe("permission-group-perm-1");
    });

    it("keeps a lone pending permission as a standalone Permission item (no grouping)", () => {
      const entries: TraceEntryData[] = [permissionEntry({ id: "perm-1", toolId: "shell" })];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      expect(result[0].kind).toBe("permission");
      expect((result[0] as ChatPermissionStreamItem).id).toBe("perm-1");
    });

    it("never groups resolved permissions and treats them as a run-breaker", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-1", toolId: "shell", status: "pending", startedAt: 1 }),
        permissionEntry({ id: "perm-2", toolId: "shell", status: "completed", startedAt: 2 }),
        permissionEntry({ id: "perm-3", toolId: "shell", status: "pending", startedAt: 3 })
      ];

      const result = buildChatStream([], entries, idle);

      // Resolved permissions are filtered out by traceEntryToStreamItem (returns
      // null for non-pending) — but they still must break the consecutive run.
      // Each surviving pending permission is on its own, so neither should be
      // wrapped in a permission_group.
      expect(result).toHaveLength(2);
      expect(result[0].kind).toBe("permission");
      expect(result[1].kind).toBe("permission");
      expect((result[0] as ChatPermissionStreamItem).id).toBe("perm-1");
      expect((result[1] as ChatPermissionStreamItem).id).toBe("perm-3");
    });

    it("treats a tool call between two pending permissions as a run-breaker", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-1", toolId: "shell", startedAt: 1 }),
        toolEntry({ id: "tool-1", toolId: "shell", status: "running", startedAt: 2 }),
        permissionEntry({ id: "perm-2", toolId: "fs.write", startedAt: 3 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(3);
      expect(result.map((item) => item.kind)).toEqual(["permission", "tool_call", "permission"]);
    });

    it("sets startedAt of the group to the FIRST pending permission's startedAt", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-a", toolId: "shell", startedAt: 100 }),
        permissionEntry({ id: "perm-b", toolId: "fs.write", startedAt: 200 }),
        permissionEntry({ id: "perm-c", toolId: "patch", startedAt: 300 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      expect((result[0] as ChatPermissionGroupStreamItem).startedAt).toBe(100);
    });

    it("groups across permission / memory variants when both are pending and consecutive", () => {
      const entries: TraceEntryData[] = [
        permissionEntry({ id: "perm-1", toolId: "shell", startedAt: 1 }),
        memoryEntry({ id: "mem-1", startedAt: 2 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      const group = result[0] as ChatPermissionGroupStreamItem;
      expect(group.kind).toBe("permission_group");
      expect(group.count).toBe(2);
      expect(group.permissionIds).toEqual(["perm-1", "mem-1"]);
    });
  });

  describe("monitor trace entries", () => {
    function monitorEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
      return {
        kind: "monitor",
        status: "running",
        title: "watch build",
        toolId: "monitor",
        startedAt: 0,
        expanded: false,
        ...overrides
      };
    }

    it("maps a running monitor trace entry to a ChatMonitorStreamItem", () => {
      const entries: TraceEntryData[] = [
        monitorEntry({ id: "mon-1", input: "tail -f build.log", startedAt: 10 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      expect(result[0]).toEqual<ChatMonitorStreamItem>({
        kind: "monitor",
        id: "mon-1",
        description: "watch build",
        status: "running",
        command: "tail -f build.log"
      });
    });

    it("maps a completed monitor with stop reason", () => {
      const entries: TraceEntryData[] = [
        monitorEntry({ id: "mon-2", status: "completed", reason: "Timeout" })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      const item = result[0] as ChatMonitorStreamItem;
      expect(item.kind).toBe("monitor");
      expect(item.status).toBe("completed");
      expect(item.stopReason).toBe("Timeout");
    });

    it("maps a failed monitor with error as lastLine", () => {
      const entries: TraceEntryData[] = [
        monitorEntry({ id: "mon-3", status: "failed", outputPreview: "spawn failed" })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result).toHaveLength(1);
      const item = result[0] as ChatMonitorStreamItem;
      expect(item.kind).toBe("monitor");
      expect(item.status).toBe("failed");
      expect(item.lastLine).toBe("spawn failed");
    });

    it("interleaves monitors with tool calls in startedAt order", () => {
      const entries: TraceEntryData[] = [
        toolEntry({ id: "tool-1", startedAt: 5 }),
        monitorEntry({ id: "mon-1", startedAt: 10 }),
        toolEntry({ id: "tool-2", startedAt: 15 })
      ];

      const result = buildChatStream([], entries, idle);

      expect(result.map((item) => item.id)).toEqual(["tool-1", "mon-1", "tool-2"]);
    });
  });

  it("never mutates the input arrays", () => {
    const messages: ChatStreamMessageInput[] = [{ role: "user", content: "hi" }];
    const entries: TraceEntryData[] = [
      toolEntry({ id: "second", startedAt: 20 }),
      toolEntry({ id: "first", startedAt: 10 })
    ];
    const messagesSnapshot = [...messages];
    const entriesSnapshot = [...entries];

    buildChatStream(messages, entries, { type: "Failed", error: "boom" });

    expect(messages).toEqual(messagesSnapshot);
    expect(entries).toEqual(entriesSnapshot);
  });
});
