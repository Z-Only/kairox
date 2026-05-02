<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import type { TraceEntryData } from "../types/trace";

const props = defineProps<{ entry: TraceEntryData }>();

const isMemory = props.entry.kind === "memory";

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
  <div :class="['permission-prompt', isMemory ? 'memory-prompt' : '']">
    <div class="permission-icon">{{ isMemory ? "🧠" : "🔑" }}</div>
    <div class="permission-body">
      <p class="permission-title">
        {{ isMemory ? "Memory Proposed" : "Permission Required" }}
      </p>
      <p class="permission-description">{{ entry.title }}</p>
      <div v-if="entry.scope" class="permission-meta">
        Scope: {{ entry.scope }}
      </div>
      <div v-if="entry.content" class="permission-meta">
        {{ entry.content }}
      </div>
      <div class="permission-meta">
        {{ isMemory ? "Store" : "Tool" }}: {{ entry.toolId }}
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
</style>
