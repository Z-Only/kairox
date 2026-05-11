<script setup lang="ts">
import { PopoverContent, PopoverRoot, PopoverTrigger } from "reka-ui";

type PopoverSide = "top" | "right" | "bottom" | "left";
type PopoverAlign = "start" | "center" | "end";

withDefaults(
  defineProps<{
    contentDataTest?: string;
    side?: PopoverSide;
    align?: PopoverAlign;
    sideOffset?: number;
  }>(),
  {
    contentDataTest: undefined,
    side: "bottom",
    align: "center",
    sideOffset: 8
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
      class="kx-popover-content"
      :data-test="contentDataTest"
      :side="side"
      :align="align"
      :side-offset="sideOffset"
    >
      <slot name="content">
        <slot />
      </slot>
    </PopoverContent>
  </PopoverRoot>
</template>
