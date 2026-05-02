<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionProjection } from "../types";
import {
  sessionState,
  setProjection,
  resetProjection
} from "../stores/session";
import { clearTrace } from "../composables/useTraceStore";

const showNewSession = ref(false);
const selectedProfile = ref("fast");
const availableProfiles = ref<string[]>([]);

async function refreshSessions() {
  try {
    sessionState.sessions = await invoke("list_sessions");
  } catch (e) {
    console.error("Failed to list sessions:", e);
  }
}

async function switchToSession(sessionId: string) {
  try {
    resetProjection();
    clearTrace();
    const projection: SessionProjection = await invoke("switch_session", {
      sessionId
    });
    setProjection(projection);
    sessionState.currentSessionId = sessionId;
    // Update currentProfile from the matching session info
    const session = sessionState.sessions.find((s) => s.id === sessionId);
    if (session) {
      sessionState.currentProfile = session.profile;
    }
  } catch (e) {
    console.error("Failed to switch session:", e);
  }
}

async function createSession() {
  try {
    const result = await invoke<{ id: string; title: string; profile: string }>(
      "start_session",
      { profile: selectedProfile.value }
    );
    await refreshSessions();
    // Update current session info immediately
    sessionState.currentSessionId = result.id;
    sessionState.currentProfile = result.profile;
    // Clear trace for the new session
    clearTrace();
    showNewSession.value = false;
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    const profiles: string[] = await invoke("list_profiles");
    availableProfiles.value = profiles;
    // Set selected profile to the one that's not currently active,
    // defaulting to the first available
    if (profiles.length > 0 && !profiles.includes(selectedProfile.value)) {
      selectedProfile.value = profiles[0];
    }
  } catch (e) {
    console.error("Failed to load profiles:", e);
  }
}

function openNewSessionDialog() {
  loadProfiles();
  showNewSession.value = true;
}
</script>

<template>
  <aside class="sessions-sidebar">
    <header class="sidebar-header">
      <h2>Sessions</h2>
      <button class="new-session-btn" @click="openNewSessionDialog">
        + New
      </button>
    </header>

    <ul v-if="sessionState.sessions.length > 0" class="session-list">
      <li
        v-for="session in sessionState.sessions"
        :key="session.id"
        :class="[
          'session-item',
          { active: session.id === sessionState.currentSessionId }
        ]"
        @click="switchToSession(session.id)"
      >
        <span class="session-indicator">●</span>
        <span class="session-title">{{ session.title }}</span>
      </li>
    </ul>
    <p v-else class="empty-hint">No sessions yet</p>

    <dialog v-if="showNewSession" class="new-session-dialog" open>
      <h3>New Session</h3>
      <label>
        Profile:
        <select v-model="selectedProfile">
          <option
            v-for="profile in availableProfiles"
            :key="profile"
            :value="profile"
          >
            {{ profile }}
          </option>
        </select>
      </label>
      <div class="dialog-actions">
        <button @click="createSession">Create</button>
        <button @click="showNewSession = false">Cancel</button>
      </div>
    </dialog>
  </aside>
</template>

<style scoped>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #d7d7d7;
}
.sidebar-header h2 {
  margin: 0;
  font-size: 14px;
}
.new-session-btn {
  font-size: 12px;
  padding: 2px 8px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
.session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
}
.session-item:hover {
  background: #f0f4f8;
}
.session-item.active {
  background: #e1ecf7;
  font-weight: 600;
}
.session-indicator {
  color: #22a06b;
  font-size: 10px;
}
.session-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 13px;
}
.new-session-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: white;
  border: 1px solid #d7d7d7;
  border-radius: 8px;
  padding: 20px;
  z-index: 100;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}
.new-session-dialog h3 {
  margin: 0 0 12px;
}
.new-session-dialog label {
  display: block;
  margin-bottom: 12px;
  font-size: 13px;
}
.new-session-dialog select {
  margin-left: 8px;
  padding: 4px;
}
.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.dialog-actions button {
  padding: 6px 12px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  background: white;
}
.dialog-actions button:first-child {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
</style>
