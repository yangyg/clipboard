<template>
  <div
    class="toggle"
    :class="{ on: modelValue }"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="ariaLabel"
    tabindex="0"
    @click="toggle"
    @keydown.enter.prevent="toggle"
    @keydown.space.prevent="toggle"
  ></div>
</template>

<script setup lang="ts">
/**
 * Reusable switch control. Reuses the global `.toggle` utility styles from
 * main.css and provides consistent role="switch" semantics + keyboard support.
 */
const props = defineProps<{
  modelValue: boolean;
  ariaLabel?: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

function toggle() {
  emit("update:modelValue", !props.modelValue);
}
</script>
