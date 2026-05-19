<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import type { McpContentBlockResponse } from "@/generated/commands";

const { t } = useI18n();
const mcp = useMcpStore();

const props = defineProps<{
  serverId: string;
}>();

const expanded = ref(false);

function toggle(): void {
  expanded.value = !expanded.value;
  if (expanded.value) {
    mcp.fetchResources(props.serverId);
  }
}

function resourceCount(): number {
  return mcp.serverResources[props.serverId]?.length ?? 0;
}

function isResourceExpanded(uri: string): boolean {
  return mcp.expandedResourceUri[props.serverId] === uri;
}

async function handleResourceClick(uri: string): Promise<void> {
  if (isResourceExpanded(uri)) {
    mcp.toggleResourceExpand(props.serverId, uri);
    return;
  }
  await mcp.readResource(props.serverId, uri);
  mcp.toggleResourceExpand(props.serverId, uri);
}

function resourceContentBlocks(uri: string): McpContentBlockResponse[] {
  return mcp.resourceContentCache[`${props.serverId}:${uri}`] ?? [];
}
</script>

<template>
  <div class="mcp-resources" :data-test="`mcp-resources-${serverId}`">
    <button
      class="mcp-resources-toggle"
      type="button"
      :aria-expanded="expanded"
      :data-test="`mcp-resources-toggle-${serverId}`"
      @click="toggle"
    >
      <span class="toggle-icon">{{ expanded ? "▼" : "▶" }}</span>
      <template v-if="mcp.loadingResources.has(serverId)">
        {{ t("mcp.loadingResources") }}
      </template>
      <template v-else>
        {{ t("mcp.resourceCount", { count: resourceCount() }) }}
      </template>
    </button>

    <KxAccordionList
      v-if="expanded"
      :aria-label="t('mcp.resourceCount', { count: resourceCount() })"
      :data-test="`mcp-resources-list-${serverId}`"
    >
      <KxAccordionState
        v-if="mcp.resourcesError[serverId]"
        tone="error"
        :data-test="`mcp-resources-error-${serverId}`"
      >
        {{ mcp.resourcesError[serverId] }}
      </KxAccordionState>
      <KxAccordionState
        v-else-if="resourceCount() === 0 && !mcp.loadingResources.has(serverId)"
        tone="empty"
        :data-test="`mcp-resources-empty-${serverId}`"
      >
        {{ t("mcp.noResources") }}
      </KxAccordionState>
      <template v-for="resource in mcp.serverResources[serverId] ?? []" :key="resource.uri">
        <KxAccordionItem
          as="button"
          :aria-expanded="isResourceExpanded(resource.uri)"
          :data-test="`mcp-resource-${serverId}-${resource.name}`"
          @click="handleResourceClick(resource.uri)"
        >
          <span class="toggle-icon">{{ isResourceExpanded(resource.uri) ? "▼" : "▶" }}</span>
          <span class="resource-name">{{ resource.name }}</span>
          <span class="resource-uri">{{ resource.uri }}</span>
          <span v-if="resource.mime_type" class="tag tag--mime">{{ resource.mime_type }}</span>
        </KxAccordionItem>
        <div
          v-if="isResourceExpanded(resource.uri)"
          class="mcp-resources-content"
          :data-test="`mcp-resource-content-${serverId}-${resource.name}`"
        >
          <div
            v-for="(block, blockIdx) in resourceContentBlocks(resource.uri)"
            :key="blockIdx"
            class="mcp-resources-content-block"
          >
            <pre v-if="block.type === 'text'" class="content-block__text">{{ block.text }}</pre>
            <img
              v-else-if="block.type === 'image'"
              :src="`data:${block.mime_type};base64,${block.data}`"
              class="content-block__image"
              :alt="resource.name"
            />
            <a
              v-else-if="block.type === 'resource'"
              class="content-block__link"
              :href="block.uri"
              >{{ block.name || block.uri }}</a
            >
          </div>
        </div>
      </template>
    </KxAccordionList>
  </div>
</template>

<style scoped>
.mcp-resources {
  width: 100%;
  margin-top: 4px;
  border-top: 1px solid var(--app-border-color, #e0e0e0);
  padding-top: 8px;
}

.mcp-resources-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--app-text-color-2, #6b7280);
  border-radius: 4px;
}

.mcp-resources-toggle:hover {
  background: var(--app-hover-color, #f3f4f6);
}

.toggle-icon {
  font-size: 10px;
}

.resource-name {
  font-weight: 600;
  white-space: nowrap;
}

.resource-uri {
  flex: 1;
  color: var(--app-text-color-2, #6b7280);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: monospace;
  font-size: 11px;
}

.tag--mime {
  background: var(--color-muted-light, #f3f4f6);
  color: var(--color-text-muted, #6b7280);
  font-size: 10px;
  text-transform: uppercase;
}

.mcp-resources-content {
  padding: 8px;
  border: 1px solid var(--app-border-color, #e0e0e0);
  border-radius: 6px;
  background: var(--app-bg-color, #f9fafb);
  margin-bottom: 4px;
}

.mcp-resources-content-block {
  max-width: 100%;
}

.content-block__text {
  margin: 0;
  padding: 8px;
  background: #1e1e1e;
  color: #d4d4d4;
  border-radius: 4px;
  font-size: 12px;
  max-height: 300px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.content-block__image {
  max-width: 100%;
  max-height: 400px;
  border-radius: 4px;
}

.content-block__link {
  color: var(--app-primary-color, #18a058);
  font-size: 12px;
  word-break: break-all;
}
</style>
