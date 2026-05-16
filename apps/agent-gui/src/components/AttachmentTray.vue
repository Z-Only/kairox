<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Attachment } from "@/composables/useChatComposer";

defineProps<{
  attachments: Attachment[];
  disabled: boolean;
}>();

const emit = defineEmits<{
  (e: "pick-files"): void;
  (e: "remove-attachment", id: string): void;
}>();

const previewAttachment = ref<Attachment | null>(null);
const previewPos = ref({ x: 0, y: 0 });
const PREVIEW_MAX_HEIGHT = 328;

const FILE_ICON_MAP: Record<string, string> = {
  pdf: "pdf",
  txt: "text",
  md: "text",
  csv: "text",
  log: "text",
  rs: "code",
  py: "code",
  ts: "code",
  js: "code",
  json: "data",
  yaml: "data",
  yml: "data",
  toml: "data",
  xml: "data",
  html: "web",
  css: "web",
  sh: "script",
  bash: "script",
  zsh: "script"
};

function isImageMime(mimeType: string): boolean {
  return mimeType.startsWith("image/");
}

function getThumbnailUrl(att: Attachment): string {
  return convertFileSrc(att.path);
}

function onThumbnailError(e: Event) {
  const img = e.target as HTMLImageElement;
  img.style.display = "none";
  const badge = img.nextElementSibling as HTMLElement | null;
  if (badge) badge.style.display = "";
}

function showPreview(att: Attachment, e: MouseEvent) {
  previewAttachment.value = att;
  clampPreviewPos(e.clientX, e.clientY);
}

function hidePreview() {
  previewAttachment.value = null;
}

function updatePreviewPos(e: MouseEvent) {
  clampPreviewPos(e.clientX, e.clientY);
}

function clampPreviewPos(clientX: number, clientY: number): void {
  let top = clientY - PREVIEW_MAX_HEIGHT / 2;
  if (top < 8) top = 8;
  if (top + PREVIEW_MAX_HEIGHT > window.innerHeight - 8) {
    top = window.innerHeight - PREVIEW_MAX_HEIGHT - 8;
  }
  previewPos.value = { x: clientX + 12, y: top };
}

function fileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function fileIconClass(name: string): string {
  const ext = fileExtension(name);
  const category = FILE_ICON_MAP[ext] || "generic";
  return `fi-${category}`;
}

function truncateFilename(name: string, maxLen = 18): string {
  if (name.length <= maxLen) return name;
  const ext = name.lastIndexOf(".");
  if (ext > 0) {
    const base = name.slice(0, ext);
    const suffix = name.slice(ext);
    const available = maxLen - suffix.length - 1;
    if (available > 4) return base.slice(0, available) + "…" + suffix;
  }
  return name.slice(0, maxLen - 1) + "…";
}
</script>

<template>
  <div v-if="attachments.length > 0" class="attachment-row" data-test="attachment-row">
    <button
      class="attach-btn attach-btn-inline"
      type="button"
      data-test="attach-file-btn"
      :aria-label="$t('chat.attachFileAria')"
      :disabled="disabled"
      @click="emit('pick-files')"
    >
      +
    </button>
    <div
      v-for="att in attachments"
      :key="att.id"
      class="attachment-chip"
      data-test="attachment-chip"
      :data-filename="att.name"
    >
      <img
        v-if="isImageMime(att.mimeType)"
        :src="getThumbnailUrl(att)"
        class="attachment-thumbnail"
        :alt="att.name"
        @error="onThumbnailError"
        @mouseenter="showPreview(att, $event)"
        @mousemove="updatePreviewPos"
        @mouseleave="hidePreview"
      />
      <span
        v-else
        :class="['attachment-type-icon', fileIconClass(att.name)]"
        :aria-label="att.mimeType"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path
            v-if="fileIconClass(att.name) === 'fi-pdf'"
            d="M6 2h8l6 6v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zm7 1.5V9h5.5L13 3.5zM7 12h10v1.5H7V12zm0 3h8v1.5H7V15zm0 3h10v1.5H7V18z"
          />
          <path
            v-else-if="fileIconClass(att.name) === 'fi-code'"
            d="M14.6 16.6l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4zm-5.2 0L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4z"
          />
          <path
            v-else-if="fileIconClass(att.name) === 'fi-data'"
            d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm0 2v2h2V5H5zm4 0v2h2V5H9zm4 0v2h2V5h-2zm4 0v2h2V5h-2zM3 9h18v2H3V9zm0 4h18v2H3v-2zm0 4h18v2H3v-2z"
          />
          <path
            v-else-if="fileIconClass(att.name) === 'fi-web'"
            d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"
          />
          <path
            v-else-if="fileIconClass(att.name) === 'fi-text'"
            d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm2 4v2h10V7H7zm0 4v2h10v-2H7zm0 4v2h7v-2H7z"
          />
          <path
            v-else-if="fileIconClass(att.name) === 'fi-script'"
            d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zM6 20V4h7v5h5v11H6zm2-6h8v2H8v-2zm0-4h4v2H8v-2z"
          />
          <path
            v-else
            d="M6 2h8l6 6v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zm7 1.5V9h5.5L13 3.5z"
          />
        </svg>
      </span>
      <span v-if="!isImageMime(att.mimeType)" class="attachment-name" :title="att.name">{{
        truncateFilename(att.name)
      }}</span>
      <button
        class="attachment-remove"
        type="button"
        :aria-label="$t('chat.removeFileAria', { name: att.name })"
        data-test="attachment-remove"
        @click="emit('remove-attachment', att.id)"
      >
        &times;
      </button>
    </div>
  </div>
  <Teleport to="body">
    <div
      v-if="previewAttachment"
      class="thumbnail-preview-overlay"
      :style="{ left: previewPos.x + 'px', top: previewPos.y + 'px' }"
      @mouseleave="hidePreview"
    >
      <img
        :src="getThumbnailUrl(previewAttachment)"
        class="thumbnail-preview-image"
        :alt="previewAttachment.name"
      />
    </div>
  </Teleport>
</template>

<style scoped>
.attachment-row {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
  overflow-x: auto;
  overflow-y: hidden;
}
.attachment-chip {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  background: var(--app-muted-surface-color, var(--app-card-color));
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  padding: 3px 6px 3px 3px;
  max-width: 200px;
}
.attachment-thumbnail {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}
.attachment-type-icon {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
  color: var(--app-primary-color);
}
.attachment-type-icon svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
}
.attachment-name {
  font-size: 12px;
  color: var(--app-text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100px;
}
.attachment-remove {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--app-muted-text-color, var(--app-text-color));
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}
.attachment-remove:hover {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 16%, transparent);
  color: var(--app-error-color, #d03050);
}
.attach-btn {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border: 1px dashed var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: transparent;
  color: var(--app-muted-text-color, var(--app-text-color));
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  line-height: 1;
}
.attach-btn:hover:not(:disabled) {
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.attach-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.attach-btn-inline {
  width: 28px;
  height: 28px;
  font-size: 16px;
}
.thumbnail-preview-overlay {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  padding: 4px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}
.thumbnail-preview-image {
  display: block;
  max-width: 320px;
  max-height: 320px;
  border-radius: 6px;
  object-fit: contain;
}
</style>
