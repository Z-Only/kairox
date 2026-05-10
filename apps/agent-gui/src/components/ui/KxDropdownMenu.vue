<script setup lang="ts">
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRoot,
  DropdownMenuTrigger
} from "reka-ui";

type DropdownSide = "top" | "right" | "bottom" | "left";
type DropdownAlign = "start" | "center" | "end";

withDefaults(
  defineProps<{
    open?: boolean;
    contentDataTest?: string;
    side?: DropdownSide;
    align?: DropdownAlign;
    sideOffset?: number;
  }>(),
  {
    open: undefined,
    contentDataTest: undefined,
    side: "bottom",
    align: "end",
    sideOffset: 6
  }
);

const emit = defineEmits<{
  "update:open": [open: boolean];
}>();
</script>

<template>
  <DropdownMenuRoot :open="open" @update:open="emit('update:open', $event)">
    <DropdownMenuTrigger as-child>
      <slot name="trigger" />
    </DropdownMenuTrigger>
    <DropdownMenuContent
      class="kx-dropdown-content"
      :data-test="contentDataTest"
      :side="side"
      :align="align"
      :side-offset="sideOffset"
    >
      <slot name="content">
        <DropdownMenuItem v-if="$slots.item" class="kx-dropdown-item" as-child>
          <slot name="item" />
        </DropdownMenuItem>
      </slot>
    </DropdownMenuContent>
  </DropdownMenuRoot>
</template>
