<template>
  <span class="brand-mark" :style="{ width: size + 'px', height: size + 'px' }" aria-hidden="true">
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="100%" height="100%">
      <rect width="32" height="32" rx="6" fill="currentColor" class="brand-bg" />
      <rect x="7" y="8" width="18" height="3" rx="1.5" fill="#fff" opacity="0.9" />
      <rect x="7" y="14" width="14" height="3" rx="1.5" fill="#fff" opacity="0.7" />
      <rect x="7" y="20" width="10" height="3" rx="1.5" fill="#fff" opacity="0.5" />
    </svg>
  </span>
</template>

<script setup lang="ts">
withDefaults(defineProps<{ size?: number }>(), { size: 22 });
</script>

<style scoped>
.brand-mark {
  display: inline-flex;
  color: var(--accent);
  flex-shrink: 0;
}

.brand-bg {
  /* fill via currentColor on parent */
}
</style>
