<template>
  <div v-if="!record.is_trashed" class="preview-actions">
    <button type="button" class="action-btn action-primary" @click="emit('paste')">
      <span class="action-icon"><AppIcon name="paste" :size="15" /></span>
      <span class="action-label">{{ $t('preview.paste') }}</span>
    </button>
    <button type="button" class="action-btn" @click="emit('paste-plain')">
      <span class="action-icon"><AppIcon name="type" :size="15" /></span>
      <span class="action-label">{{ $t('preview.plainText') }}</span>
    </button>
    <button type="button" class="action-btn action-fav" :class="{ 'action-active': record.is_favorite }" @click="emit('favorite')">
      <span class="action-icon"><AppIcon name="star" :size="15" :fill="record.is_favorite ? 'currentColor' : 'none'" /></span>
      <span class="action-label">{{ record.is_favorite ? $t('preview.favorited') : $t('preview.favorite') }}</span>
    </button>
    <button type="button" class="action-btn action-pin" :class="{ 'action-pinned': pinnedDisplay }" @click="emit('pin')">
      <span class="action-icon"><AppIcon name="pin" :size="15" :fill="pinnedDisplay ? 'currentColor' : 'none'" /></span>
      <span class="action-label">{{ pinnedDisplay ? $t('preview.pinned') : $t('preview.pin') }}</span>
    </button>
    <button type="button" class="action-btn action-icon-only danger" :aria-label="$t('preview.deleteBtn')" :title="$t('preview.deleteBtn')" @click="emit('delete')">
      <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
    </button>
  </div>
  <div v-else class="preview-actions trash-actions">
    <button type="button" class="action-btn action-primary" @click="emit('restore')">
      <span class="action-icon"><AppIcon name="restore" :size="15" /></span>
      <span class="action-label">{{ $t('preview.restoreBtn') }}</span>
    </button>
    <button type="button" class="action-btn action-icon-only danger" :aria-label="$t('preview.permanentDelete')" :title="$t('preview.permanentDelete')" @click="emit('permanent-delete')">
      <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
    </button>
  </div>
</template>

<script setup lang="ts">
import type { ClipboardRecord } from "../types";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  record: ClipboardRecord;
  pinnedDisplay: boolean;
}>();

const emit = defineEmits<{
  paste: [];
  "paste-plain": [];
  favorite: [];
  pin: [];
  delete: [];
  restore: [];
  "permanent-delete": [];
}>();
</script>

<style scoped>
.preview-actions {
  padding: var(--space-2) var(--space-5) var(--space-5);
  display: grid;
  grid-template-columns: 1.5fr repeat(3, 1fr) auto;
  gap: var(--space-2);
}

.action-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-1);
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-default, var(--border-subtle));
  background: var(--bg-card, var(--bg-surface));
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), transform var(--transition-fast);
  font-family: inherit;
}

.action-btn:hover { background: var(--accent-soft); border-color: color-mix(in srgb, var(--accent) 25%, transparent); }
.action-btn:active { transform: scale(0.96); }
.action-btn:hover .action-label, .action-btn:hover .action-icon { color: var(--accent-text); }
.action-btn.action-primary { background: var(--accent); border-color: var(--accent); color: var(--text-on-accent, #fff); }
.action-btn.action-primary .action-label, .action-btn.action-primary .action-icon { color: var(--text-on-accent, #fff); }
.action-btn.action-primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }
.action-btn.action-primary:hover .action-label, .action-btn.action-primary:hover .action-icon { color: var(--text-on-accent, #fff); }
.action-btn.action-fav:hover { background: var(--warning-soft); border-color: color-mix(in srgb, var(--warning) 20%, transparent); }
.action-btn.action-fav:hover .action-label, .action-btn.action-fav:hover .action-icon { color: var(--warning); }
.action-btn.action-active { background: var(--warning-soft); border-color: color-mix(in srgb, var(--warning) 20%, transparent); }
.action-btn.action-active .action-label, .action-btn.action-active .action-icon { color: var(--warning); }
.action-btn.action-fav.action-active:hover { background: color-mix(in srgb, var(--warning) 28%, transparent); border-color: color-mix(in srgb, var(--warning) 45%, transparent); }
.action-btn.action-pin:hover { background: var(--pin-soft); border-color: color-mix(in srgb, var(--pin) 20%, transparent); }
.action-btn.action-pin:hover .action-label, .action-btn.action-pin:hover .action-icon { color: var(--pin); }
.action-btn.action-pinned { background: var(--pin-soft); border-color: color-mix(in srgb, var(--pin) 20%, transparent); }
.action-btn.action-pinned .action-label, .action-btn.action-pinned .action-icon { color: var(--pin); }
.action-btn.action-pin.action-pinned:hover { background: color-mix(in srgb, var(--pin) 28%, transparent); border-color: color-mix(in srgb, var(--pin) 45%, transparent); }
.action-btn.action-icon-only { padding: var(--space-3); min-width: 42px; }
.action-btn.danger { color: var(--danger); }
.action-btn.danger .action-icon { color: var(--danger); }
.action-btn.danger:hover { background: var(--danger-soft); border-color: color-mix(in srgb, var(--danger) 20%, transparent); }
.action-btn.danger:hover .action-label, .action-btn.danger:hover .action-icon { color: var(--danger); }
.trash-actions { grid-template-columns: 1.5fr auto; }
.action-icon { font-size: var(--text-2xl); display: flex; align-items: center; justify-content: center; color: var(--text-secondary); transition: color var(--transition-fast); }
.action-label { font-size: var(--text-sm); font-weight: 600; color: var(--text-secondary); transition: color var(--transition-fast); }

@media (max-width: 720px) {
  .preview-actions:not(.trash-actions) { grid-template-columns: 1.4fr 1fr 1fr 1fr auto; }
}

@media (max-width: 560px) {
  .preview-actions:not(.trash-actions) { grid-template-columns: 1.4fr 1fr 1fr auto; }
  .preview-actions:not(.trash-actions) .action-btn:nth-child(4) .action-label { display: none; }
}
</style>
