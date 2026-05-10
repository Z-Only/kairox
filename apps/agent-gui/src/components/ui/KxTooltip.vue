<script setup lang="ts">
import { TooltipContent, TooltipProvider, TooltipRoot, TooltipTrigger } from "reka-ui";

type TooltipSide = "top" | "right" | "bottom" | "left";

withDefaults(
  defineProps<{
    text: string;
    side?: TooltipSide;
  }>(),
  {
    side: "top"
  }
);
</script>

<template>
  <TooltipProvider>
    <TooltipRoot>
      <TooltipTrigger as-child>
        <slot />
      </TooltipTrigger>
      <TooltipContent class="kx-tooltip-content" :side="side" :side-offset="6">
        {{ text }}
      </TooltipContent>
    </TooltipRoot>
  </TooltipProvider>
</template>
