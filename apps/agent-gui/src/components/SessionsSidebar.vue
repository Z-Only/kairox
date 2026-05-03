<script setup lang="ts">
import { ref, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionProjection, ProfileDetail } from "../types";
import {
  sessionState,
  setProjection,
  resetProjection,
  deleteSession,
  renameSession
} from "../stores/session";
import { applyTraceEvent, clearTrace } from "../composables/useTraceStore";
import { refreshTaskGraph, clearTaskGraph } from "../stores/taskGraph";
import ConfirmDialog from "./ConfirmDialog.vue";

const showNewSession = ref(false);
const showDeleteDialog = ref(false);
const deleteTargetId = ref("");
const deleteTargetTitle = ref("");
const selectedProfile = ref("fast");
const availableProfiles = ref<ProfileDetail[]>([]);
const editingSessionId = ref<string | null>(null);
const editingTitle = ref("");
const profileDropdownOpen = ref(false);
const renameInput = ref<HTMLInputElement | null>(null);

async function refreshSessions() {
  try {
    sessionState.sessions = await invoke("list_sessions");
  } catch (e) {
    console.error("Failed to list sessions:", e);
  }
}

async function switchToSession(sessionId: string) {
  if (editingSessionId.value) return;
  try {
    resetProjection();
    clearTrace();
    clearTaskGraph();
    const projection: SessionProjection = await invoke("switch_session", {
      sessionId
    });
    setProjection(projection);
    sessionState.currentSessionId = sessionId;
    refreshTaskGraph(sessionId);
    const session = sessionState.sessions.find((s) => s.id === sessionId);
    if (session) {
      sessionState.currentProfile = session.profile;
    }
    try {
      const traceStrings: string[] = await invoke("get_trace", { sessionId });
      for (const jsonStr of traceStrings) {
        try {
          applyTraceEvent(JSON.parse(jsonStr));
        } catch {
          // Skip malformed trace entries
        }
      }
    } catch (e) {
      console.error("Failed to load trace for session:", e);
    }
  } catch (e) {
    console.error("Failed to switch session:", e);
  }
}

async function createSession() {
  try {
    const result = await invoke<{
      id: string;
      title: string;
      profile: string;
    }>("start_session", { profile: selectedProfile.value });
    await refreshSessions();
    sessionState.currentSessionId = result.id;
    sessionState.currentProfile = result.profile;
    resetProjection();
    clearTrace();
    showNewSession.value = false;
    profileDropdownOpen.value = false;
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    availableProfiles.value = (await invoke(
      "get_profile_info"
    )) as ProfileDetail[];
    if (availableProfiles.value.length > 0) {
      selectedProfile.value = availableProfiles.value[0].alias;
    }
  } catch (e) {
    console.error("Failed to load profiles:", e);
    // Fallback: try to get just profile names
    try {
      const names: string[] = await invoke("list_profiles");
      availableProfiles.value = names.map((name) => ({
        alias: name,
        provider: "unknown",
        model_id: "unknown",
        local: false,
        has_api_key: false
      }));
      if (names.length > 0) {
        selectedProfile.value = names[0];
      }
    } catch {
      // Ignore fallback failure
    }
  }
}

function openNewSessionDialog() {
  loadProfiles();
  showNewSession.value = true;
}

function startRename(sessionId: string, currentTitle: string) {
  editingSessionId.value = sessionId;
  editingTitle.value = currentTitle;
  nextTick(() => {
    renameInput.value?.focus();
    renameInput.value?.select();
  });
}

async function confirmRename() {
  if (editingSessionId.value && editingTitle.value.trim()) {
    await renameSession(editingSessionId.value, editingTitle.value.trim());
  }
  editingSessionId.value = null;
}

function cancelRename() {
  editingSessionId.value = null;
}

function promptDelete(sessionId: string, title: string) {
  deleteTargetId.value = sessionId;
  deleteTargetTitle.value = title;
  showDeleteDialog.value = true;
}

async function confirmDelete() {
  await deleteSession(deleteTargetId.value);
  showDeleteDialog.value = false;
}

function cancelDelete() {
  showDeleteDialog.value = false;
}

function selectProfile(alias: string) {
  selectedProfile.value = alias;
  profileDropdownOpen.value = false;
}

function keyIcon(hasApiKey: boolean): string {
  return hasApiKey ? "🔑" : "🚫";
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

        <!-- Inline rename mode -->
        <template v-if="editingSessionId === session.id">
          <input
            ref="renameInput"
            v-model="editingTitle"
            class="rename-input"
            @keydown.enter="confirmRename"
            @keydown.escape="cancelRename"
            @blur="confirmRename"
            @click.stop
          />
        </template>

        <!-- Normal display mode -->
        <template v-else>
          <span class="session-title">{{ session.title }}</span>
          <span class="session-actions">
            <button
              class="action-btn"
              title="Rename"
              @click.stop="startRename(session.id, session.title)"
            >
              ✏️
            </button>
            <button
              class="action-btn action-delete"
              title="Delete"
              @click.stop="promptDelete(session.id, session.title)"
            >
              🗑️
            </button>
          </span>
        </template>
      </li>
    </ul>
    <p v-else class="empty-hint">No sessions yet</p>

    <!-- New Session Dialog -->
    <dialog v-if="showNewSession" class="new-session-dialog" open>
      <h3>New Session</h3>
      <label>
        Profile:
        <div class="profile-dropdown">
          <button
            class="profile-trigger"
            @click="profileDropdownOpen = !profileDropdownOpen"
          >
            {{ selectedProfile }}
            <span class="caret">▼</span>
          </button>
          <div v-if="profileDropdownOpen" class="profile-menu">
            <div
              v-for="p in availableProfiles"
              :key="p.alias"
              :class="[
                'profile-option',
                { selected: p.alias === selectedProfile }
              ]"
              @click="selectProfile(p.alias)"
            >
              <div class="profile-info">
                <span class="profile-alias">{{ p.alias }}</span>
                <span
                  class="profile-detail"
                  :title="`${p.provider} · ${p.model_id}`"
                >
                  {{ p.provider }} · {{ p.model_id }}
                </span>
              </div>
              <span class="profile-key">{{ keyIcon(p.has_api_key) }}</span>
            </div>
          </div>
        </div>
      </label>
      <div class="dialog-actions">
        <button @click="createSession">Create</button>
        <button
          @click="
            showNewSession = false;
            profileDropdownOpen = false;
          "
        >
          Cancel
        </button>
      </div>
    </dialog>

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      v-if="showDeleteDialog"
      :title="`Delete '${deleteTargetTitle}'?`"
      message="This session's conversation history will be permanently removed after 7 days."
      confirm-label="Delete"
      :confirm-danger="true"
      @confirm="confirmDelete"
      @cancel="cancelDelete"
    />
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
  position: relative;
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
  flex-shrink: 0;
}
.session-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-actions {
  display: none;
  gap: 4px;
  flex-shrink: 0;
}
.session-item:hover .session-actions {
  display: flex;
}
.action-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 13px;
  padding: 2px;
  border-radius: 3px;
  line-height: 1;
}
.action-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}
.action-delete:hover {
  background: rgba(204, 51, 51, 0.1);
}
.rename-input {
  flex: 1;
  border: 1px solid #0077cc;
  border-radius: 3px;
  padding: 2px 4px;
  font-size: 13px;
  outline: none;
  font-family: inherit;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 13px;
}

/* New Session Dialog */
.new-session-dialog {
  min-width: 340px;
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

/* Profile Dropdown */
.profile-dropdown {
  position: relative;
  margin-top: 6px;
}
.profile-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  padding: 6px 10px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  background: white;
  cursor: pointer;
  font-size: 13px;
  text-align: left;
}
.caret {
  font-size: 10px;
  color: #777;
}
.profile-menu {
  position: absolute;
  top: 100%;
  left: 0;
  min-width: 320px;
  background: white;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  z-index: 10;
  max-height: 200px;
  overflow-y: auto;
}
.profile-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  cursor: pointer;
  font-size: 12px;
}
.profile-option:hover {
  background: #f0f4f8;
}
.profile-option.selected {
  background: #e1ecf7;
  font-weight: 600;
}
.profile-alias {
  font-weight: 600;
  font-size: 13px;
}
.profile-detail {
  color: #666;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.profile-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.profile-key {
  flex-shrink: 0;
  font-size: 11px;
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
  font-size: 13px;
}
.dialog-actions button:first-child {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
</style>
