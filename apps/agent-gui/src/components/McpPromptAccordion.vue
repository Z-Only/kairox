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

    <KxAccordionList
      v-if="expanded"
      :aria-label="t('mcp.promptCount', { count: promptCount() })"
      :data-test="`mcp-prompts-list-${serverId}`"
    >
      <KxAccordionState
        v-if="mcp.promptsError[serverId]"
        tone="error"
        :data-test="`mcp-prompts-error-${serverId}`"
      >
        {{ mcp.promptsError[serverId] }}
      </KxAccordionState>
      <KxAccordionState
        v-else-if="promptCount() === 0 && !mcp.loadingPrompts.has(serverId)"
        tone="empty"
        :data-test="`mcp-prompts-empty-${serverId}`"
      >
        {{ t("mcp.noPrompts") }}
      </KxAccordionState>
      <KxAccordionItem
        v-for="prompt in mcp.serverPrompts[serverId] ?? []"
        :key="prompt.name"
        :data-test="`mcp-prompt-${serverId}-${prompt.name}`"
      >
        <span class="prompt-name">{{ prompt.name }}</span>
        <span class="tag tag--mime">{{
          t("mcp.argumentsCount", { count: prompt.argument_count })
        }}</span>
        <span v-if="prompt.description" class="prompt-desc">{{ prompt.description }}</span>
      </KxAccordionItem>
    </KxAccordionList>
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
