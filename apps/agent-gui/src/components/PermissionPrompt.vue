<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMcpStore } from "@/stores/mcp";
import { useAgentsStore } from "@/stores/agents";
import type { TraceEntryData } from "../types/trace";

const props = defineProps<{ entry: TraceEntryData }>();
const mcp = useMcpStore();
const agents = useAgentsStore();

const isMemory = props.entry.kind === "memory";

/** Detect MCP tools by their "mcp.{server_id}.{tool_name}" format. */
const isMcpTool = computed(() => props.entry.toolId?.startsWith("mcp."));

/** Extract the server ID from an MCP tool ID like "mcp.github.list_repos". */
const mcpServerId = computed(() => {
  if (!isMcpTool.value) return null;
  const parts = props.entry.toolId!.split(".");
  // "mcp.{server_id}.{tool_name}" — server_id may contain dots, but
  // conventionally the second segment is the server ID.
  return parts.length >= 3 ? parts[1] : null;
});

/** Whether this MCP server is already trusted. */
const isServerTrusted = computed(() => {
  if (!mcpServerId.value) return false;
  return mcp.trustedServerIds.includes(mcpServerId.value);
});

/** Checkbox state for "Trust this server". */
const trustChecked = ref(false);

/** The source agent label if available from the entry's rawEvent. */
const sourceAgentLabel = computed(() => {
  if (!props.entry.rawEvent) return null;
  try {
    const event = JSON.parse(props.entry.rawEvent);
    const agentId = event?.source_agent_id;
    if (agentId && agentId !== "agent_system") {
      return agents.agentLabel(agentId);
    }
  } catch {
    // Ignore parse errors
  }
  return null;
});

async function allow() {
  try {
    await invoke("resolve_permission", {
      requestId: props.entry.id,
      decision: "grant"
    });
    if (isMcpTool.value && trustChecked.value && mcpServerId.value) {
      await mcp.trustServer(mcpServerId.value);
    }
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
  <div :class="['permission-prompt', isMemory ? 'memory-prompt' : '']">
    <div class="permission-icon">{{ isMemory ? "🧠" : "🔑" }}</div>
    <div class="permission-body">
      <p class="permission-title">
        {{ isMemory ? "Memory Proposed" : "Permission Required" }}
        <span v-if="sourceAgentLabel" class="permission-agent-badge">
          {{ sourceAgentLabel }}
        </span>
      </p>
      <p class="permission-description">{{ entry.title }}</p>
      <div v-if="entry.scope" class="permission-meta">Scope: {{ entry.scope }}</div>
      <div v-if="entry.content" class="permission-meta">
        {{ entry.content }}
      </div>
      <div class="permission-meta">{{ isMemory ? "Store" : "Tool" }}: {{ entry.toolId }}</div>
      <!-- MCP-specific UI -->
      <div v-if="isMcpTool && mcpServerId" class="mcp-permission-info">
        <div class="mcp-server-label">
          MCP Server: <strong>{{ mcpServerId }}</strong>
          <span v-if="isServerTrusted" class="mcp-trusted-badge">✅ Trusted</span>
        </div>
        <label v-if="!isServerTrusted" class="mcp-trust-check">
          <input v-model="trustChecked" type="checkbox" />
          Trust this server for future requests
        </label>
      </div>
    </div>
    <div class="permission-actions">
      <button class="btn-allow" @click="allow">
        {{ isMemory ? "Accept" : "Allow" }}
      </button>
      <button class="btn-deny" @click="deny">
        {{ isMemory ? "Reject" : "Deny" }}
      </button>
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
.memory-prompt {
  background: #f0faf0;
  border-color: #a0d8a0;
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
  display: flex;
  align-items: center;
  gap: 6px;
}
.permission-agent-badge {
  font-size: 10px;
  font-weight: 600;
  color: white;
  background: #0077cc;
  border-radius: 3px;
  padding: 0 4px;
  line-height: 16px;
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
  overflow-wrap: anywhere;
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
.mcp-permission-info {
  margin-top: 6px;
  padding: 4px 8px;
  background: #f0f4ff;
  border-radius: 4px;
  border: 1px solid #c8d6f0;
}
.mcp-server-label {
  font-size: 11px;
  color: #444;
}
.mcp-trusted-badge {
  margin-left: 6px;
  color: #22a06b;
  font-size: 11px;
}
.mcp-trust-check {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  font-size: 11px;
  color: #555;
  cursor: pointer;
}
.mcp-trust-check input[type="checkbox"] {
  margin: 0;
  cursor: pointer;
}
</style>
