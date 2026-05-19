<script setup lang="ts">
defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
    closeLabel?: string;
    width?: string;
    bodyDataTest?: string;
    closeOnBackdrop?: boolean;
    showClose?: boolean;
  }>(),
  {
    closeLabel: "Close",
    width: "520px",
    closeOnBackdrop: true,
    showClose: true
  }
);

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <KxModal
    v-bind="$attrs"
    :open="open"
    :title="title"
    :description="description"
    :close-label="closeLabel"
    :width="width"
    :body-data-test="bodyDataTest"
    :close-on-backdrop="closeOnBackdrop"
    :show-close="showClose"
    @close="emit('close')"
  >
    <template v-if="$slots.header" #header>
      <slot name="header" />
    </template>

    <slot />

    <template v-if="$slots.footer" #footer>
      <slot name="footer" />
    </template>
  </KxModal>
</template>
