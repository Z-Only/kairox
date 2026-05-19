<script setup lang="ts">
import type { SidebarRenameController } from "@/composables/sidebar/useSidebarRename";
import type { SessionInfoResponse } from "@/types";

const { t } = useI18n();

defineProps<{
  sessions: SessionInfoResponse[];
  activeSessionId: string | null;
  pendingDeleteSessionId: string | null;
  rename: SidebarRenameController;
  createSession: () => Promise<void> | void;
  switchToSession: (sessionId: string) => Promise<void> | void;
  requestDeleteSession: (sessionId: string) => Promise<void> | void;
}>();
</script>

<template>
  <section class="sidebar-section" data-test="sessions-section">
    <div class="section-heading">
      <h3>{{ t("sessions.header") }}</h3>
      <div class="section-actions">
        <KxTooltip :text="t('sessions.newButton')">
          <KxIconButton
            :label="t('sessions.newButton')"
            :title="t('sessions.newButton')"
            data-test="new-session-btn"
            @click="createSession"
          >
            <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
              <path d="M9.25 3h1.5v6.25H17v1.5h-6.25V17h-1.5v-6.25H3v-1.5h6.25V3Z" />
            </svg>
          </KxIconButton>
        </KxTooltip>
      </div>
    </div>
    <div class="sidebar-section-scroll" data-test="sessions-scroll-region">
      <template v-if="sessions.length > 0">
        <!-- Kept hand-rolled because NListItem #suffix slot cannot express the current compact row layout. -->
        <ul class="session-list">
          <li
            v-for="item in sessions"
            :key="item.id"
            :class="['session-item', { active: item.id === activeSessionId }]"
            data-test="session-item"
            @click="rename.editingId.value ? undefined : switchToSession(item.id)"
          >
            <span class="session-indicator">●</span>

            <template v-if="rename.editingId.value === item.id">
              <KxEditableLabel
                v-model="rename.title.value"
                :input-ref="(el) => rename.bindInput(el, item.id)"
                input-data-test="session-rename-input"
                confirm-data-test="session-rename-confirm"
                :confirm-label="t('common.confirm')"
                @confirm="rename.confirm"
                @cancel="rename.cancel"
                @click.stop
              />
            </template>

            <template v-else>
              <span class="session-title truncate" :title="item.title">{{ item.title }}</span>
              <span class="row-actions session-actions">
                <KxTooltip :text="t('sessions.renameTitle')">
                  <KxIconButton
                    :label="t('sessions.renameTitle')"
                    data-test="session-rename-btn"
                    @click.stop="rename.start(item.id, item.title)"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path
                        d="M13.7 2.3a1 1 0 0 1 1.4 0l2.6 2.6a1 1 0 0 1 0 1.4l-9.45 9.45L4 16l.25-4.25L13.7 2.3Zm.7 1.4-8.7 8.7-.12 2.02 2.02-.12 8.7-8.7-1.9-1.9Z"
                      />
                    </svg>
                  </KxIconButton>
                </KxTooltip>
                <KxTooltip
                  :text="
                    pendingDeleteSessionId === item.id
                      ? t('sessions.confirmArchive')
                      : t('sessions.archive')
                  "
                >
                  <KxIconButton
                    :label="
                      pendingDeleteSessionId === item.id
                        ? t('sessions.confirmArchive')
                        : t('sessions.archive')
                    "
                    :class="{ 'confirm-action': pendingDeleteSessionId === item.id }"
                    data-test="session-archive-btn"
                    @click.stop="requestDeleteSession(item.id)"
                  >
                    <svg
                      v-if="pendingDeleteSessionId === item.id"
                      viewBox="0 0 20 20"
                      aria-hidden="true"
                      focusable="false"
                    >
                      <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                    </svg>
                    <svg v-else viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path
                        d="M4 3h12v3H4V3Zm1.5 1.5v.75h9v-.75h-9ZM5 7h10v8.5A1.5 1.5 0 0 1 13.5 17h-7A1.5 1.5 0 0 1 5 15.5V7Zm3 2v1.5h4V9H8Z"
                      />
                    </svg>
                  </KxIconButton>
                </KxTooltip>
              </span>
            </template>
          </li>
        </ul>
      </template>
      <div v-else class="empty-state empty-hint" data-test="sessions-empty">
        {{ t("sessions.emptyHint") }}
      </div>
    </div>
  </section>
</template>
