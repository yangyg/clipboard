<template>
  <div
    ref="rootEl"
    :id="`${optionIdPrefix}${record.id}`"
    class="record-item"
    role="option"
    :aria-selected="batchMode ? checked : selected"
    :tabindex="tabbable ? 0 : -1"
    :class="{
      selected: selected && !batchMode,
      'batch-mode': batchMode,
      'batch-checked': batchMode && checked,
      'is-text': record.content_type === 'text',
      'is-link': record.content_type === 'link',
      'is-code': record.content_type === 'code',
      'is-image': record.content_type === 'image',
      'is-file': record.content_type === 'file',
      'is-new': isNew,
      'is-leaving': isLeaving,
    }"
    :data-record-id="record.id"
    @click="emit('click', record.id, $event)"
    @contextmenu.prevent="emit('context-menu', $event, record)"
    @keydown.enter.prevent.stop="emit('activate', record.id)"
    @keydown.space.prevent="emit('click', record.id)"
  >
    <div v-if="batchMode" class="record-checkbox" :class="{ checked }" aria-hidden="true">
      <span v-if="checked">✓</span>
    </div>

    <div v-if="rowColor(record)" class="record-color-swatch" :style="{ background: rowColor(record)! }" :title="rowColor(record)!" aria-hidden="true" />
    <div v-else class="record-type-icon type-chip" :class="record.content_type" aria-hidden="true">
      <TypeIcon :type="record.content_type" :size="14" />
    </div>

    <div class="record-body">
      <div v-if="record.content_type === 'image' && thumb" class="record-image-tile" aria-hidden="true">
        <img class="record-thumb" :src="thumb" alt="" loading="lazy" decoding="async" />
      </div>
      <div v-else class="record-title" :title="recordTitleAttr(record, t)">
        <AppIcon v-if="hasAlias" name="edit" :size="12" class="alias-mark" aria-hidden="true" />
        <span v-html="previewHtml(record, searchQuery, t)"></span>
      </div>
      <div class="record-meta">
        <span class="record-time">{{ formatTime(record.created_at, t) }}</span>
        <span class="record-source" v-html="sourceLabelHtml(record, searchQuery, t, sourceOverrides) ?? escapeHtml(resolveSourceLabel(record.source_app, record.source_name, t, sourceOverrides))"></span>
        <span v-if="deviceOrigin" class="record-device" :title="deviceTooltip"><AppIcon name="cloud" :size="12" />{{ deviceOrigin }}</span>
        <span v-if="record.content_type === 'image' && record.width && record.height" class="record-dims">{{ record.width }}×{{ record.height }}</span>
        <span v-if="record.is_sensitive" class="record-sensitive">{{ $t('record.sensitive') }}</span>
      </div>
    </div>

    <div class="record-actions" @click.stop>
      <template v-if="trashFilter">
        <button type="button" class="record-action-btn" :aria-label="$t('record.restoreRecord')" :title="$t('record.restoreRecord')" @click="emit('restore', record.id)"><AppIcon name="restore" :size="13" /></button>
        <button type="button" class="record-action-btn danger" :aria-label="$t('record.permanentDelete')" :title="$t('record.permanentDelete')" @click="emit('delete', record)"><AppIcon name="trash" :size="13" /></button>
      </template>
      <template v-else>
        <button type="button" class="record-action-btn" :aria-label="$t('record.pasteLabel')" :title="$t('record.pasteLabel')" @click="emit('paste', record.id)"><AppIcon name="paste" :size="13" /></button>
        <button type="button" class="record-action-btn action-fav" :class="{ starred: record.is_favorite }" :aria-label="record.is_favorite ? $t('record.unfavorite') : $t('record.favorite')" :title="record.is_favorite ? $t('record.unfavorite') : $t('record.favorite')" @click="emit('favorite', record.id)"><AppIcon name="star" :size="13" :fill="record.is_favorite ? 'currentColor' : 'none'" /></button>
        <button type="button" class="record-action-btn action-pin" :class="{ active: pinned }" :aria-label="pinned ? $t('record.unpin') : $t('record.pin')" :title="pinned ? $t('record.unpin') : $t('record.pin')" @click="emit('toggle-pin', record)"><AppIcon name="pin" :size="13" :fill="pinned ? 'currentColor' : 'none'" /></button>
        <button type="button" class="record-action-btn danger" :aria-label="$t('record.deleteRecord')" :title="$t('record.deleteRecord')" @click="emit('delete', record)"><AppIcon name="trash" :size="13" /></button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord } from "../types";
import { escapeHtml } from "../utils/highlightSearch";
import { formatTime, previewHtml, recordAlias, recordTitleAttr, rowColor, sourceLabelHtml } from "../utils/recordFormatting";
import { resolveDeviceLabel, resolveDeviceTooltip, resolveSourceLabel } from "../utils/sourceBadge";
import { useSettingsStore } from "../stores/settings";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";

const props = withDefaults(
  defineProps<{
    record: ClipboardRecord;
    thumb?: string | null;
    batchMode: boolean;
    checked: boolean;
    selected: boolean;
    tabbable: boolean;
    trashFilter: boolean;
    pinned: boolean;
    isNew: boolean;
    isLeaving: boolean;
    searchQuery: string;
    sourceOverrides: Record<string, string>;
    /** Reports this row's element to the virtualizer (list layout measuring). */
    measureRow?: (id: number, el: HTMLElement | null) => void;
    /** Listbox option id prefix. Dock overlay uses a distinct prefix to avoid duplicates. */
    optionIdPrefix?: string;
  }>(),
  { optionIdPrefix: "record-option-" },
);

const rootEl = ref<HTMLElement | null>(null);

onMounted(() => {
  props.measureRow?.(props.record.id, rootEl.value);
});
onUnmounted(() => {
  props.measureRow?.(props.record.id, null);
});

const hasAlias = computed(() => recordAlias(props.record).length > 0);
const settingsStore = useSettingsStore();
const deviceOrigin = computed(() =>
  resolveDeviceLabel(
    props.record,
    settingsStore.settings.webdav_device_names,
    settingsStore.settings.webdav_device_id,
    t,
  ),
);
const deviceTooltip = computed(() =>
  resolveDeviceTooltip(
    props.record,
    settingsStore.settings.webdav_device_names,
    settingsStore.settings.webdav_device_id,
    t,
  ),
);

const emit = defineEmits<{
  click: [id: number, event?: MouseEvent];
  activate: [id: number];
  "context-menu": [event: MouseEvent, record: ClipboardRecord];
  paste: [id: number];
  favorite: [id: number];
  "toggle-pin": [record: ClipboardRecord];
  delete: [record: ClipboardRecord];
  restore: [id: number];
}>();

const { t } = useI18n();
</script>

<style>
.record-item { --row-accent: var(--accent); padding: 10px 12px; margin: 0 4px 2px; cursor: pointer; border-radius: var(--radius-sm); transition: background var(--transition-fast), opacity var(--transition-fast), transform var(--transition-fast); display: flex; align-items: flex-start; gap: var(--space-3); position: relative; border: 1px solid transparent; background: transparent; box-shadow: none; }
.record-item.is-text { --row-accent: var(--type-text); }
.record-item.is-code { --row-accent: var(--type-code); }
.record-item.is-link { --row-accent: var(--type-link); }
.record-item.is-image { --row-accent: var(--type-image); }
.record-item.is-file { --row-accent: var(--type-file); }
.record-item:hover { background: var(--bg-hover); }
.record-item.selected { background: color-mix(in srgb, var(--accent) 14%, transparent); }
.record-item.is-leaving { opacity: 0; transform: translateX(-4px); pointer-events: none; }
.record-item.is-new::before { content: ""; position: absolute; inset: 0; border-radius: inherit; background: color-mix(in srgb, var(--accent) 18%, transparent); pointer-events: none; animation: row-flash var(--animation-flash) forwards; }
.record-item:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.record-item.batch-mode { padding-left: 32px; }
.record-checkbox { position: absolute; left: 10px; top: 16px; width: 14px; height: 14px; border: 1.5px solid var(--text-tertiary); border-radius: var(--radius-xs); display: flex; align-items: center; justify-content: center; font-size: var(--text-xs); color: transparent; transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast); flex-shrink: 0; }
.record-checkbox.checked { background: var(--accent); border-color: var(--accent); color: var(--text-on-accent); }
.record-type-icon { width: 32px; height: 32px; border-radius: var(--radius-sm, 6px); display: flex; align-items: center; justify-content: center; flex-shrink: 0; margin-top: 1px; }
.record-color-swatch { width: 32px; height: 32px; border-radius: var(--radius-sm, 6px); flex-shrink: 0; margin-top: 1px; border: 1px solid var(--border-default); box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 10%, transparent); }
.record-image-tile { width: 64px; height: 48px; border-radius: var(--radius-sm, 6px); overflow: hidden; border: 1px solid var(--border-subtle); background: var(--bg-elevated); }
.record-thumb { width: 100%; height: 100%; object-fit: cover; display: block; }
.record-body { flex: 1; min-width: 0; }
.record-title { font-size: var(--text-base, 0.8125rem); font-weight: 500; color: var(--text-primary); line-height: 1.4; display: flex; align-items: flex-start; gap: 4px; min-width: 0; }
.record-title > span { flex: 1 1 auto; min-width: 0; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; text-overflow: ellipsis; white-space: normal; word-break: break-word; overflow-wrap: anywhere; }
.record-title .alias-mark { flex-shrink: 0; margin-top: 2px; color: var(--text-tertiary); text-decoration: none; }
.record-item.is-link .record-title { color: var(--type-link); text-decoration: underline; text-decoration-color: color-mix(in srgb, var(--type-link) 35%, transparent); text-underline-offset: 2px; }
.record-item.is-link .record-title .alias-mark { text-decoration: none; color: var(--text-tertiary); }
.record-item.is-code .record-title { font-family: var(--font-mono); font-weight: 400; font-size: var(--text-md, 0.75rem); }
.record-meta { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); margin-top: 6px; font-size: var(--text-sm, 0.6875rem); color: var(--text-tertiary); }
.record-time { white-space: nowrap; }
.record-source { display: inline-flex; align-items: center; min-width: 0; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.record-device { display: inline-flex; align-items: center; gap: 4px; padding: 0 6px; border: 1px solid var(--border-subtle, var(--border-default)); border-radius: 999px; line-height: 1.35; white-space: nowrap; opacity: 0.9; }
.record-dims { white-space: nowrap; opacity: 0.85; }
.record-sensitive { font-size: var(--text-xs, 0.625rem); font-weight: 600; color: var(--sensitive); background: var(--sensitive-soft); padding: 1px 6px; border-radius: 4px; }
.record-actions { display: flex; align-items: center; gap: 2px; flex-shrink: 0; opacity: 0; pointer-events: none; transition: opacity var(--transition-fast); margin-top: -2px; }
.record-item:hover .record-actions, .record-item:focus-within .record-actions, .record-item.selected .record-actions, .record-actions:has(.active), .record-actions:has(.starred) { opacity: 1; pointer-events: auto; }
.record-item:not(:hover):not(:focus-within):not(.selected) .record-action-btn:not(.active):not(.starred) { display: none; }
.record-action-btn { width: 28px; height: 28px; border: none; border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); cursor: pointer; display: inline-flex; align-items: center; justify-content: center; transition: background var(--transition-fast), color var(--transition-fast), transform var(--transition-instant); }
.record-action-btn:active { transform: scale(0.88); }
.record-action-btn:hover { background: var(--accent-soft); color: var(--accent-text); }
.record-action-btn.action-fav:hover { background: var(--warning-soft); color: var(--warning); }
.record-action-btn.action-pin:hover { background: var(--pin-soft); color: var(--pin); }
.record-action-btn.danger:hover { background: var(--danger-soft); color: var(--danger); }
.record-action-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 0; }
.record-action-btn.action-pin.active { color: var(--pin); transform: rotate(-20deg); }
.record-action-btn.starred { color: var(--warning); }
.record-action-btn.active, .record-action-btn.starred { opacity: 1; }

.view-grid .record-item { display: flex; flex-direction: column; align-items: stretch; min-width: 0; max-width: 100%; overflow: hidden; margin: 0; padding: 10px; gap: 6px; height: calc(132px * var(--ui-font-scale, 1)); max-height: calc(132px * var(--ui-font-scale, 1)); box-sizing: border-box; border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); background: var(--bg-surface); }
.view-grid .record-item:hover { background: var(--bg-hover); border-color: var(--border-default); box-shadow: none; }
.view-grid .record-item.selected { background: color-mix(in srgb, var(--accent) 12%, var(--bg-surface)); border-color: color-mix(in srgb, var(--accent) 32%, transparent); box-shadow: none; }
.view-grid .record-item.is-image { height: calc(140px * var(--ui-font-scale, 1)); max-height: calc(140px * var(--ui-font-scale, 1)); }
.view-grid .record-item.batch-mode { padding: 10px; }
.view-grid .record-checkbox { left: auto; right: var(--space-2); top: var(--space-2); z-index: 3; width: 18px; height: 18px; border-radius: var(--radius-sm); background: var(--bg-elevated); border-color: var(--border-default); box-shadow: var(--shadow-sm); }
.view-grid .record-checkbox.checked { background: var(--accent); border-color: var(--accent); color: var(--text-on-accent); }
.view-grid .record-item.batch-mode .record-type-icon { margin-left: 0; }
.view-grid .record-item.batch-checked { border-color: color-mix(in srgb, var(--accent) 40%, transparent); background: color-mix(in srgb, var(--accent) 10%, var(--bg-surface)); box-shadow: none; }
.view-grid .record-item.batch-mode .record-actions { display: none; }
.view-grid .record-item.is-image .record-type-icon { display: none; }
.view-grid .record-type-icon, .view-grid .record-color-swatch { width: 28px; height: 28px; margin-top: 0; }
.view-grid .record-body { display: flex; flex-direction: column; flex: 1 1 auto; width: 100%; min-width: 0; min-height: 0; gap: var(--space-1); overflow: hidden; }
.view-grid .record-image-tile { order: -1; width: 100%; height: 72px; max-height: 72px; flex: 0 0 72px; overflow: hidden; }
.view-grid .record-title { flex: 1 1 auto; min-height: 0; max-height: calc(1.35em * 2); display: flex; align-items: flex-start; gap: 4px; line-height: 1.35; }
.view-grid .record-title > span { flex: 1 1 auto; min-width: 0; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; text-overflow: ellipsis; white-space: normal; word-break: break-word; overflow-wrap: anywhere; }
.view-grid .record-title .alias-mark { margin-top: 2px; }
.view-grid .record-meta { display: flex; flex-wrap: nowrap; align-items: center; margin-top: auto; gap: 6px; width: 100%; min-width: 0; overflow: hidden; flex-shrink: 0; }
.view-grid .record-time { flex-shrink: 0; }
.view-grid .record-source { flex: 1 1 auto; min-width: 0; max-width: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.view-grid .record-dims { display: none; }
.view-grid .record-sensitive { flex-shrink: 0; max-width: 3.5rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.view-grid .record-actions { position: absolute; top: var(--space-2); right: var(--space-2); margin: 0; z-index: 2; max-width: calc(100% - 12px); overflow: hidden; background: color-mix(in srgb, var(--bg-surface) 94%, transparent); border-radius: var(--radius-sm); padding: 1px; box-shadow: var(--shadow-sm); }
.view-grid .record-action-btn { width: 26px; height: 26px; flex-shrink: 0; }

@keyframes row-flash { from { opacity: 1; } to { opacity: 0; } }
</style>
