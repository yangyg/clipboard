<template>
  <div class="preview-header">
    <div class="preview-type-row">
      <div class="preview-type-icon type-chip" :class="record.content_type" :title="$t('preview.contentType', { type: typeLabel })">
        <TypeIcon :type="record.content_type" :size="14" />
      </div>
      <div class="preview-heading">
        <div class="preview-name">
          <button v-if="!record.is_trashed" type="button" class="preview-alias-btn" :title="recordAlias ? $t('preview.editAlias') : $t('preview.setAlias')" @click="emit('edit-alias')">
            <AppIcon name="edit" :size="11" />
            <span>{{ recordAlias || $t('preview.setAlias') }}</span>
          </button>
        </div>
        <div class="preview-meta-line">
          <div class="preview-meta-row">
            <span class="preview-meta-item" :title="sourceTitle">{{ $t('preview.source', { name: sourceLabel }) }}</span>
            <span v-if="deviceOrigin" class="preview-meta-item preview-device">{{ deviceOrigin }}</span>
            <span class="preview-meta-item">{{ $t('preview.createdAt', { time: formatDateTime(record.created_at) }) }}</span>
          </div>
          <div class="preview-meta-more">
            <span class="preview-meta-item">{{ $t('preview.updatedAt', { time: formatDateTime(record.updated_at) }) }}</span>
            <template v-if="record.content_type === 'image' && record.width && record.height">
              <span class="preview-meta-item">{{ $t('preview.dimensions', { w: record.width, h: record.height }) }}</span>
            </template>
            <template v-else>
              <span class="preview-meta-item">{{ $t('preview.charCount', { count: record.content_len ?? record.content.length }) }}</span>
            </template>
            <template v-if="record.content_html">
              <span class="preview-meta-item">{{ $t('preview.richTextTitle') }}</span>
            </template>
            <span class="preview-meta-item">{{ $t('preview.pasteCount', { count: record.copy_count }) }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord } from "../types";
import { useSettingsStore } from "../stores/settings";
import { buildSourceOverrides, resolveDeviceLabel, resolveSourceLabel } from "../utils/sourceBadge";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";

const props = defineProps<{
  record: ClipboardRecord;
  typeLabel: string;
  recordAlias: string;
  formatDateTime: (iso: string) => string;
}>();

const { t } = useI18n();
const settingsStore = useSettingsStore();

const sourceLabel = computed(() =>
  resolveSourceLabel(
    props.record.source_app,
    props.record.source_name,
    t,
    buildSourceOverrides(settingsStore.settings.source_name_overrides),
  ),
);

const sourceTitle = computed(() => {
  const raw = (props.record.source_app || "").trim();
  if (!raw) return t("record.systemClipboard");
  return sourceLabel.value === raw
    ? t("record.sourceTooltip", { app: raw })
    : t("record.sourceTooltip", { app: `${sourceLabel.value} (${raw})` });
});

const deviceOrigin = computed(() =>
  resolveDeviceLabel(
    props.record,
    settingsStore.settings.webdav_device_names,
    settingsStore.settings.webdav_device_id,
    t,
  ),
);

const emit = defineEmits<{
  "edit-alias": [];
}>();
</script>

<style scoped>
.preview-header { padding: 14px 20px; border-bottom: 1px solid var(--border-light, var(--border-subtle)); }
.preview-type-row { display: flex; align-items: center; gap: 10px; }
.preview-type-icon { width: 40px; height: 40px; border-radius: var(--radius-md, 10px); display: flex; align-items: center; justify-content: center; font-size: var(--text-lg); font-weight: 600; flex-shrink: 0; }
.preview-heading { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.preview-name { display: flex; align-items: center; min-width: 0; }
.preview-alias-btn { display: inline-flex; align-items: center; gap: 6px; max-width: 100%; padding: 0; border: none; background: none; font-family: inherit; font-size: var(--text-lg); font-weight: 600; color: var(--text-primary); cursor: pointer; text-align: left; }
.preview-alias-btn span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.preview-alias-btn:hover { color: var(--accent-text); text-decoration: underline; text-underline-offset: 3px; }
.preview-meta-line { display: flex; flex-direction: column; gap: 2px; font-size: var(--text-sm); color: var(--text-muted, var(--text-tertiary)); line-height: 1.35; overflow: hidden; }
.preview-meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 1px 14px; min-width: 0; }
.preview-meta-row > span,
.preview-meta-more > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.preview-device { padding: 0 6px; border: 1px solid var(--border-subtle, var(--border-default)); border-radius: 999px; line-height: 1.35; }
.preview-meta-more { display: flex; flex-wrap: wrap; align-items: center; gap: 1px 14px; min-width: 0; opacity: 0; visibility: hidden; max-height: 0; overflow: hidden; transition: opacity var(--transition-fast), max-height var(--transition-smooth), visibility var(--transition-fast); }
.preview-heading:hover .preview-meta-more,
.preview-heading:focus-within .preview-meta-more { opacity: 1; visibility: visible; max-height: 3.5rem; }
</style>
