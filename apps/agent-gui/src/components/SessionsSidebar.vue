<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useDialog } from "naive-ui";
import type { ProfileDetail } from "../types";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const dialog = useDialog();

const session = useSessionStore();
const route = useRoute();
const router = useRouter();

// The active session is derived from the URL (`/workbench/:sessionId?`),
// so navigation through the sidebar drives the router and the router
// drives the store via WorkbenchView's watcher.
const activeSessionId = computed<string | null>(() => {
  const v = route.params.sessionId;
  const id = Array.isArray(v) ? v[0] : v;
  return id ?? session.currentSessionId;
});

const showNewSession = ref(false);
const selectedProfile = ref("fast");
const availableProfiles = ref<ProfileDetail[]>([]);
const editingSessionId = ref<string | null>(null);
const editingTitle = ref("");
const profileDropdownOpen = ref(false);
const renameInput = ref<HTMLInputElement | null>(null);

async function switchToSession(sessionId: string) {
  if (editingSessionId.value) return;
  if (sessionId === activeSessionId.value) return;
  try {
    await router.push({ name: "workbench", params: { sessionId } });
  } catch (e) {
    console.error("Failed to navigate to session:", e);
  }
}

async function createSession() {
  try {
    const result = await session.createSession(selectedProfile.value);
    showNewSession.value = false;
    profileDropdownOpen.value = false;
    await router.push({ name: "workbench", params: { sessionId: result.id } });
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    availableProfiles.value = (await invoke("get_profile_info")) as ProfileDetail[];
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

// Functional ref for the rename `<input>` inside `v-for`. Vue 3 treats a
// string `ref="renameInput"` inside `v-for` as an array (one entry per
// iteration); the previous code happened to work because
// `editingSessionId === item.id` ensures only one `<input>` is rendered at
// any time, but it was a latent foot-gun. The functional ref pins the
// variable to the single editing row explicitly.
function bindRenameInput(el: Element | null, itemId: string) {
  if (editingSessionId.value === itemId) {
    renameInput.value = (el as HTMLInputElement) ?? null;
  }
}

async function confirmRename() {
  if (editingSessionId.value && editingTitle.value.trim()) {
    await session.renameSession(editingSessionId.value, editingTitle.value.trim());
  }
  editingSessionId.value = null;
}

function cancelRename() {
  editingSessionId.value = null;
}

function promptDelete(sessionId: string, title: string) {
  // The destructive confirmation is portal-rendered by NaiveUI under
  // `<NDialogProvider>` (mounted in `AppLayout.vue`). The view layer no
  // longer owns visibility state — the dialog hook does, and a positive
  // click delegates to the existing `session.deleteSession` action.
  dialog.warning({
    title: t("common.confirm"),
    content: t("sessions.deleteConfirm", { title }),
    positiveText: t("common.delete"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      void session.deleteSession(sessionId);
    }
  });
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
  <aside class="sessions-sidebar" data-test="sessions-sidebar">
    <header class="sidebar-header">
      <h2>{{ t("sessions.header") }}</h2>
      <NButton
        size="tiny"
        type="primary"
        class="new-session-btn"
        data-test="new-session-btn"
        @click="openNewSessionDialog"
      >
        {{ t("sessions.newButtonPrefix") }}{{ t("sessions.newButton") }}
      </NButton>
    </header>

    <NScrollbar v-if="session.sessions.length > 0" class="session-scroll">
      <!-- Kept hand-rolled because hover-only .session-actions cannot be expressed via NListItem #suffix slot. -->
      <ul class="session-list">
        <li
          v-for="item in session.sessions"
          :key="item.id"
          :class="['session-item', { active: item.id === activeSessionId }]"
          data-test="session-item"
          @click="switchToSession(item.id)"
        >
          <span class="session-indicator">●</span>

          <!-- Inline rename mode -->
          <template v-if="editingSessionId === item.id">
            <input
              :ref="(el) => bindRenameInput(el as Element | null, item.id)"
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
            <span class="session-title">{{ item.title }}</span>
            <span class="session-actions">
              <NButton
                quaternary
                size="tiny"
                class="action-btn"
                :title="t('sessions.renameTitle')"
                @click.stop="startRename(item.id, item.title)"
              >
                ✏️
              </NButton>
              <NButton
                quaternary
                size="tiny"
                type="error"
                class="action-btn action-delete"
                :title="t('sessions.deleteTitle')"
                data-test="session-delete-btn"
                @click.stop="promptDelete(item.id, item.title)"
              >
                🗑️
              </NButton>
            </span>
          </template>
        </li>
      </ul>
    </NScrollbar>
    <NEmpty
      v-else
      size="small"
      class="empty-hint"
      :description="t('sessions.emptyHint')"
      data-test="sessions-empty"
    />

    <!-- New Session Dialog (kept as native <dialog> per Task 5 NIT #8 — out of
         scope for Task 7 spec §5.5 mapping). -->
    <dialog v-if="showNewSession" class="new-session-dialog" open>
      <h3>{{ t("sessions.newDialogTitle") }}</h3>
      <label>
        {{ t("sessions.profileLabel") }}
        <div class="profile-dropdown">
          <button class="profile-trigger" @click="profileDropdownOpen = !profileDropdownOpen">
            {{ selectedProfile }}
            <span class="caret">▼</span>
          </button>
          <div v-if="profileDropdownOpen" class="profile-menu">
            <div
              v-for="p in availableProfiles"
              :key="p.alias"
              :class="['profile-option', { selected: p.alias === selectedProfile }]"
              @click="selectProfile(p.alias)"
            >
              <div class="profile-info">
                <span class="profile-alias">{{ p.alias }}</span>
                <span class="profile-detail" :title="`${p.provider} · ${p.model_id}`">
                  {{ p.provider }} · {{ p.model_id }}
                </span>
              </div>
              <span class="profile-key">{{ keyIcon(p.has_api_key) }}</span>
            </div>
          </div>
        </div>
      </label>
      <div class="dialog-actions">
        <button data-test="create-session-btn" @click="createSession">
          {{ t("sessions.createButton") }}
        </button>
        <button
          @click="
            showNewSession = false;
            profileDropdownOpen = false;
          "
        >
          {{ t("sessions.cancelButton") }}
        </button>
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
