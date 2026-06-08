<script setup lang="ts">
const props = defineProps<{
  src: string;
  alt?: string;
}>();

const isOpen = ref(false);

function open() {
  isOpen.value = true;
}

function close() {
  isOpen.value = false;
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close();
  }
}

whenever(isOpen, (opened) => {
  if (opened) {
    document.addEventListener("keydown", handleKeydown);
  } else {
    document.removeEventListener("keydown", handleKeydown);
  }
});

onUnmounted(() => {
  document.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <img :src="props.src" :alt="props.alt" class="lightbox-thumbnail" @click="open" />

  <Teleport to="body">
    <Transition name="lightbox-fade">
      <div v-if="isOpen" class="lightbox-overlay" @click="close">
        <img :src="props.src" :alt="props.alt" class="lightbox-image" @click.stop />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.lightbox-thumbnail {
  max-width: 100%;
  max-height: 300px;
  border-radius: 6px;
  cursor: zoom-in;
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.lightbox-thumbnail:hover {
  opacity: 0.9;
  transform: scale(1.02);
}

.lightbox-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.85);
  cursor: zoom-out;
}

.lightbox-image {
  max-width: 90vw;
  max-height: 90vh;
  object-fit: contain;
  border-radius: 4px;
  cursor: default;
}

.lightbox-fade-enter-active,
.lightbox-fade-leave-active {
  transition: opacity 0.25s ease;
}

.lightbox-fade-enter-from,
.lightbox-fade-leave-to {
  opacity: 0;
}
</style>
