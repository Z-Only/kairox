<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

type LightboxMedia =
  | {
      kind: "image";
      src: string;
      alt: string;
      caption: string;
    }
  | {
      kind: "diagram";
      svg: string;
      caption: string;
    };

const media = ref<LightboxMedia | null>(null);
const dialogRef = ref<HTMLElement | null>(null);
let previousFocus: HTMLElement | null = null;

function findMediaTarget(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) {
    return null;
  }

  const mediaTarget = target.closest<HTMLElement>(
    ".vp-doc .mermaid, .vp-doc img, .screenshot-grid img"
  );

  if (!mediaTarget || mediaTarget.closest("a")) {
    return null;
  }

  return mediaTarget;
}

function findCaption(target: HTMLElement): string {
  const caption = target.closest("figure")?.querySelector("figcaption")?.textContent?.trim();
  return caption ?? "";
}

function openImage(image: HTMLImageElement): void {
  const src = image.currentSrc || image.src;

  if (!src) {
    return;
  }

  media.value = {
    kind: "image",
    src,
    alt: image.alt,
    caption: findCaption(image) || image.alt
  };
}

function openDiagram(container: HTMLElement): void {
  const svg = container.querySelector("svg");

  if (!svg) {
    return;
  }

  const clone = svg.cloneNode(true) as SVGElement;
  clone.removeAttribute("height");
  clone.setAttribute("width", "100%");
  clone.setAttribute("focusable", "false");
  clone.setAttribute("aria-hidden", "true");

  media.value = {
    kind: "diagram",
    svg: new XMLSerializer().serializeToString(clone),
    caption: findCaption(container)
  };
}

function openMedia(target: HTMLElement): void {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  if (target instanceof HTMLImageElement) {
    openImage(target);
  } else {
    openDiagram(target);
  }

  void nextTick(() => dialogRef.value?.focus());
}

function close(): void {
  media.value = null;
  previousFocus?.focus();
  previousFocus = null;
}

function onDocumentClick(event: MouseEvent): void {
  const target = findMediaTarget(event.target);

  if (!target) {
    return;
  }

  event.preventDefault();
  openMedia(target);
}

function onDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && media.value) {
    event.preventDefault();
    close();
  }
}

onMounted(() => {
  document.addEventListener("click", onDocumentClick);
  document.addEventListener("keydown", onDocumentKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div v-if="media" class="kairox-media-lightbox" role="presentation" @click.self="close">
      <section
        ref="dialogRef"
        class="kairox-media-lightbox__dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Enlarged media preview"
        tabindex="-1"
      >
        <button
          class="kairox-media-lightbox__close"
          type="button"
          aria-label="Close enlarged media preview"
          @click="close"
        >
          &times;
        </button>

        <div class="kairox-media-lightbox__viewport">
          <img v-if="media.kind === 'image'" :src="media.src" :alt="media.alt" />
          <div v-else class="kairox-media-lightbox__svg" v-html="media.svg" />
        </div>

        <p v-if="media.caption" class="kairox-media-lightbox__caption">
          {{ media.caption }}
        </p>
      </section>
    </div>
  </Teleport>
</template>
