<script setup lang="ts">
import { PopoverContent, PopoverRoot, PopoverTrigger } from "reka-ui";

type PopoverSide = "top" | "right" | "bottom" | "left";
type PopoverAlign = "start" | "center" | "end";

withDefaults(
  defineProps<{
    contentDataTest?: string;
    contentClass?: string;
    side?: PopoverSide;
    align?: PopoverAlign;
    sideOffset?: number;
    width?: string;
    maxHeight?: string;
  }>(),
  {
    contentDataTest: undefined,
    contentClass: undefined,
    side: "bottom",
    align: "center",
    sideOffset: 8,
    width: undefined,
    maxHeight: undefined
  }
);

const open = defineModel<boolean>("open", { default: false });
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <slot name="trigger" />
    </PopoverTrigger>
    <PopoverContent
      :class="['kx-popover-content', contentClass]"
      :data-test="contentDataTest"
      :side="side"
      :align="align"
      :side-offset="sideOffset"
      :style="{
        '--kx-popover-width': width,
        '--kx-popover-max-height': maxHeight
      }"
    >
      <slot name="content">
        <slot />
      </slot>
    </PopoverContent>
  </PopoverRoot>
</template>
