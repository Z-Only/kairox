<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    title: string;
    description?: string | null;
    headingLevel?: 3 | 4;
    tagsLabel?: string;
    descriptionLines?: number;
  }>(),
  {
    description: undefined,
    headingLevel: 3,
    tagsLabel: undefined,
    descriptionLines: 2
  }
);

const headingTag = computed(() => `h${props.headingLevel}`);
const descriptionStyle = computed(() => ({
  "--settings-item-description-lines": String(props.descriptionLines)
}));
</script>

<template>
  <div class="settings-item-summary">
    <div class="settings-item-summary__header">
      <component :is="headingTag" class="settings-item-summary__title">
        {{ title }}
      </component>
      <div v-if="$slots.tags" class="settings-item-summary__tags" :aria-label="tagsLabel">
        <slot name="tags" />
      </div>
    </div>
    <p
      v-if="description"
      class="settings-item-summary__description settings-item-summary__description--clamp"
      :style="descriptionStyle"
    >
      {{ description }}
    </p>
    <slot />
  </div>
</template>

<style scoped>
.settings-item-summary {
  min-width: 0;
  display: grid;
  gap: 8px;
}

.settings-item-summary__header {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.settings-item-summary__title {
  min-width: 0;
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-item-summary__tags {
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.settings-item-summary__description {
  margin: 0;
  color: var(--app-text-color-2);
  line-height: 1.45;
}

.settings-item-summary__description--clamp {
  display: -webkit-box;
  overflow: hidden;
  text-overflow: ellipsis;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: var(--settings-item-description-lines);
}

.settings-item-summary :deep(code) {
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
