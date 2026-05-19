<script setup lang="ts">
import { TooltipContent, TooltipProvider, TooltipRoot, TooltipTrigger } from "reka-ui";

type TooltipSide = "top" | "right" | "bottom" | "left";

withDefaults(
  defineProps<{
    text: string;
    contentDataTest?: string;
    side?: TooltipSide;
    sideOffset?: number;
  }>(),
  {
    contentDataTest: undefined,
    side: "top",
    sideOffset: 6
  }
);

const open = defineModel<boolean>("open", { default: undefined });
</script>

<template>
  <TooltipProvider>
    <TooltipRoot v-model:open="open">
      <TooltipTrigger as-child>
        <slot />
      </TooltipTrigger>
      <TooltipContent
        class="kx-tooltip-content"
        :data-test="contentDataTest"
        :side="side"
        :side-offset="sideOffset"
      >
        {{ text }}
      </TooltipContent>
    </TooltipRoot>
  </TooltipProvider>
</template>
