<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";

const { t } = useI18n();
const mcp = useMcpStore();

const props = defineProps<{
  serverId: string;
}>();

const expanded = ref(false);

function toggle(): void {
  expanded.value = !expanded.value;
  if (expanded.value) {
    mcp.fetchPrompts(props.serverId);
  }
}

function promptCount(): number {
  return mcp.serverPrompts[props.serverId]?.length ?? 0;
}
</script>

<template>
  <div class="mcp-prompts" :data-test="`mcp-prompts-${serverId}`">
    <button
      class="mcp-prompts-toggle"
      type="button"
      :aria-expanded="expanded"
      :data-test="`mcp-prompts-toggle-${serverId}`"
      @click="toggle"
    >
      <span class="toggle-icon">{{ expanded ? "▼" : "▶" }}</span>
      <template v-if="mcp.loadingPrompts.has(serverId)">
        {{ t("mcp.loadingPrompts") }}
      </template>
      <template v-else>
        {{ t("mcp.promptCount", { count: promptCount() }) }}
      </template>
    </button>

    <div v-if="expanded" class="mcp-prompts-list">
      <KxStateBlock v-if="mcp.promptsError[serverId]" tone="error" compact>
        {{ mcp.promptsError[serverId] }}
      </KxStateBlock>
      <KxStateBlock
        v-else-if="promptCount() === 0 && !mcp.loadingPrompts.has(serverId)"
        tone="empty"
        compact
      >
        {{ t("mcp.noPrompts") }}
      </KxStateBlock>
      <div
        v-for="prompt in mcp.serverPrompts[serverId] ?? []"
        :key="prompt.name"
        class="mcp-prompts-row"
        :data-test="`mcp-prompt-${serverId}-${prompt.name}`"
      >
        <span class="prompt-name">{{ prompt.name }}</span>
        <span class="tag tag--mime">{{
          t("mcp.argumentsCount", { count: prompt.argument_count })
        }}</span>
        <span v-if="prompt.description" class="prompt-desc">{{ prompt.description }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mcp-prompts {
  width: 100%;
  margin-top: 4px;
  border-top: 1px solid var(--app-border-color, #e0e0e0);
  padding-top: 8px;
}

.mcp-prompts-toggle {
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

.mcp-prompts-toggle:hover {
  background: var(--app-hover-color, #f3f4f6);
}

.toggle-icon {
  font-size: 10px;
}

.mcp-prompts-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 6px;
}

.mcp-prompts-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  font-size: 12px;
}

.prompt-name {
  font-weight: 600;
  font-family: monospace;
}

.prompt-desc {
  flex: 1;
  color: var(--app-text-color-2, #6b7280);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag--mime {
  background: var(--color-muted-light, #f3f4f6);
  color: var(--color-text-muted, #6b7280);
  font-size: 10px;
  text-transform: uppercase;
}
</style>
