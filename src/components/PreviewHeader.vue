<template>
  <div class="preview-header">
    <div class="preview-type-row">
      <div class="preview-type-icon type-chip" :class="record.content_type" :title="$t('preview.contentType', { type: typeLabel })">
        <TypeIcon :type="record.content_type" :size="14" />
      </div>
      <div class="preview-heading">
        <div class="preview-name" :title="$t('preview.contentType', { type: typeLabel })">{{ typeLabel }}</div>
        <button v-if="!record.is_trashed" type="button" class="preview-alias-btn" :class="{ 'has-alias': !!recordAlias }" :title="recordAlias ? $t('preview.editAlias') : $t('preview.setAlias')" @click="emit('edit-alias')">
          <AppIcon name="edit" :size="11" />
          <span>{{ recordAlias || $t('preview.setAlias') }}</span>
        </button>
        <div class="preview-meta-line">
          <SourceBadge :source-app="record.source_app" :source-name="record.source_name" />
          <span class="meta-sep" aria-hidden="true">·</span>
          <span :title="$t('preview.createdAt', { time: formatDateTime(record.created_at) })">{{ formatDateTime(record.created_at) }}</span>
          <template v-if="record.content_type === 'image' && record.width && record.height">
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="$t('preview.dimensions', { w: record.width, h: record.height })">{{ record.width }}×{{ record.height }}</span>
          </template>
          <template v-else>
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="$t('preview.charCount', { count: record.content_len ?? record.content.length })">{{ record.content_len ?? record.content.length }} {{ $t('common.chars') }}</span>
          </template>
          <template v-if="record.content_html">
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="$t('preview.richTextTitle')">{{ $t('preview.richText') }}</span>
          </template>
          <span class="meta-sep" aria-hidden="true">·</span>
          <span :title="$t('preview.pasteCountTitle', { count: record.copy_count })">{{ $t('preview.pasteCount', { count: record.copy_count }) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ClipboardRecord } from "../types";
import AppIcon from "./icons/AppIcon.vue";
import SourceBadge from "./SourceBadge.vue";
import TypeIcon from "./icons/TypeIcon.vue";

defineProps<{
  record: ClipboardRecord;
  typeLabel: string;
  recordAlias: string;
  formatDateTime: (iso: string) => string;
}>();

const emit = defineEmits<{
  "edit-alias": [];
}>();
</script>

<style scoped>
.preview-header { padding: 14px 20px; border-bottom: 1px solid var(--border-light, var(--border-subtle)); }
.preview-type-row { display: flex; align-items: center; gap: 10px; }
.preview-type-icon { width: 40px; height: 40px; border-radius: var(--radius-md, 10px); display: flex; align-items: center; justify-content: center; font-size: var(--text-lg); font-weight: 600; flex-shrink: 0; }
.preview-heading { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.preview-name { font-size: var(--text-lg); font-weight: 600; color: var(--text-primary); }
.preview-alias-btn { display: inline-flex; align-items: center; gap: var(--space-1); margin-top: 2px; max-width: 100%; padding: 0; border: none; background: none; font-family: inherit; font-size: var(--text-md); font-weight: 500; color: var(--text-tertiary); cursor: pointer; text-align: left; }
.preview-alias-btn span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.preview-alias-btn:hover { color: var(--accent-text); }
.preview-alias-btn.has-alias { color: var(--text-secondary); }
.preview-meta-line { display: flex; flex-wrap: wrap; align-items: center; gap: 0 2px; font-size: var(--text-sm); color: var(--text-muted, var(--text-tertiary)); line-height: 1.35; overflow: hidden; }
.preview-meta-line > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.meta-sep { flex-shrink: 0; margin: 0 var(--space-1); opacity: 0.7; }
</style>
