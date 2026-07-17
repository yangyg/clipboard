<script setup lang="ts">
import { computed } from "vue";
import AppIcon, { type AppIconName } from "./AppIcon.vue";
import type { ContentType } from "../../types";

const props = withDefaults(
  defineProps<{
    type: ContentType | string;
    size?: number | string;
  }>(),
  { size: 14 }
);

const name = computed<AppIconName>(() => {
  switch (props.type) {
    case "code":
      return "code";
    case "link":
      return "link";
    case "image":
      return "image";
    case "file":
      return "file";
    case "sensitive":
      return "warning";
    case "text":
    default:
      return "type";
  }
});
</script>

<template>
  <AppIcon :name="name" :size="size" />
</template>
