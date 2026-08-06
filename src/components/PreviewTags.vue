<template>
  <div class="preview-tags">
    <div class="tags-label">{{ $t('preview.tags') }}</div>
    <div class="tags-list">
      <span v-for="tag in record.tags" :key="tag" class="tag-chip" :style="{ background: getTagBg(tag), color: getTagColor(tag) }">
        <span class="tag-dot" :style="{ background: getTagColor(tag) }"></span>
        {{ tag }}
        <button type="button" class="tag-remove" @click.stop="emit('remove-tag', tag)" :aria-label="$t('preview.removeTag')" :title="$t('preview.removeTag')">
          <AppIcon name="close" :size="10" />
        </button>
      </span>
      <button class="tag-add-btn" @click="emit('open-assign')"><AppIcon name="plus" :size="12" /> {{ $t('preview.addTag') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ClipboardRecord } from "../types";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  record: ClipboardRecord;
  getTagBg: (tag: string) => string;
  getTagColor: (tag: string) => string;
}>();

const emit = defineEmits<{
  "remove-tag": [tag: string];
  "open-assign": [];
}>();
</script>

<style scoped>
.preview-tags { padding: var(--space-2) var(--space-5) var(--space-4); }
.tags-label { font-size: var(--text-md); font-weight: 600; color: var(--text-primary); margin-bottom: var(--space-2); }
.tags-list { display: flex; flex-wrap: wrap; gap: var(--space-2); }
.tag-chip { display: flex; align-items: center; gap: 5px; padding: var(--space-1) var(--space-3); border-radius: var(--radius-xl); font-size: var(--text-md); font-weight: 500; }
.tag-dot { width: 6px; height: 6px; border-radius: var(--radius-pill); }
.tag-remove { width: 14px; height: 14px; border-radius: var(--radius-pill); background: transparent; color: inherit; opacity: 0.6; font-size: var(--text-xs); display: flex; align-items: center; justify-content: center; cursor: pointer; transition: opacity var(--transition-fast); padding: 0; margin-left: 2px; border: none; }
.tag-remove:hover { opacity: 1; }
.tag-add-btn { display: flex; align-items: center; gap: var(--space-1); padding: var(--space-1) var(--space-3); border-radius: var(--radius-xl); font-size: var(--text-md); color: var(--text-muted, var(--text-tertiary)); cursor: pointer; border: 1px dashed var(--border-default, var(--border-subtle)); background: transparent; transition: color var(--transition-fast), border-color var(--transition-fast); }
.tag-add-btn:hover { color: var(--accent-text); border-color: var(--accent); }
</style>
