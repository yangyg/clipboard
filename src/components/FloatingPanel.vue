<template>
  <div class="floating-panel panel-surface">
    <!-- Header -->
    <div class="panel-header">
      <div class="panel-title-row">
        <div>
          <div class="panel-title">Clipboard</div>
          <CaptureStatus compact />
        </div>
        <div class="header-actions">
        <button
          type="button"
          class="icon-btn"
          :class="{ active: clipboardStore.batchMode }"
          :title="$t('panel.batch')"
          :aria-label="$t('panel.batch')"
          :aria-pressed="clipboardStore.batchMode"
          @click="toggleBatchMode"
        ><AppIcon name="batch" :size="ICON_MD" /></button>
        <button
          type="button"
          class="icon-btn"
          :class="{ active: clipboardStore.activeFilter === 'favorites' }"
          :title="$t('panel.favorites')"
          :aria-label="$t('panel.favorites')"
          :aria-pressed="clipboardStore.activeFilter === 'favorites'"
          @click="clipboardStore.setFilter(clipboardStore.activeFilter === 'favorites' ? 'all' : 'favorites')"
        ><AppIcon name="star" :size="ICON_MD" :fill="clipboardStore.activeFilter === 'favorites' ? 'currentColor' : 'none'" /></button>
        <button
          type="button"
          class="icon-btn"
          :class="{ active: clipboardStore.trashFilter }"
          :title="$t('panel.trash')"
          :aria-label="$t('panel.trash')"
          :aria-pressed="clipboardStore.trashFilter"
          @click="toggleTrash"
        ><AppIcon name="trash" :size="ICON_MD" /></button>
        <button type="button" class="icon-btn" :title="$t('panel.settings')" :aria-label="$t('panel.settings')" @click="emit('openSettings')"><AppIcon name="settings" :size="ICON_MD" /></button>
        </div>
      </div>
      <SearchBar />
    </div>

    <!-- Filter Tabs (hidden in trash) -->
    <div v-if="!clipboardStore.trashFilter" class="filter-row">
      <button
        v-for="tab in FILTER_TABS"
        :key="tab.key"
        class="filter-tab"
        :class="{ active: clipboardStore.activeFilter === tab.key }"
        @click="clipboardStore.setFilter(tab.key)"
      >
        {{ $t(tab.labelKey) }}
        <span class="filter-count">{{ clipboardStore.filterCounts[tab.key] }}</span>
      </button>
    </div>
    <div v-else class="trash-banner">
      <span>{{ $t('panel.trashBanner', { count: clipboardStore.trashCount }) }}</span>
      <button
        v-if="clipboardStore.trashCount > 0"
        class="empty-trash-link"
        @click="onEmptyTrash"
      >{{ $t('panel.emptyTrash') }}</button>
    </div>

    <!-- Body -->
    <div class="panel-body">
      <Transition name="fade">
        <BatchBar v-if="clipboardStore.batchMode" />
      </Transition>

      <RecordList />
    </div>
  </div>
</template>

<script setup lang="ts">
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import CaptureStatus from "./CaptureStatus.vue";
import BatchBar from "./BatchBar.vue";
import AppIcon from "./icons/AppIcon.vue";
import { useClipboardStore } from "../stores/clipboard";
import type { FilterTab } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useBatchActions } from "../composables/useBatchActions";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

/** Toolbar icon size: sm 13 / md 15 / lg 18 */
const ICON_MD = 15;

const emit = defineEmits<{
  openSettings: [];
  close: [];
}>();

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode } = useBatchActions();

useClipboardHotkeys({
  onClose: () => emit("close"),
  allowCloseOnEscape: true,
});

const FILTER_TABS: { key: FilterTab; labelKey: string }[] = [
  { key: "all", labelKey: "filter.all" },
  { key: "text", labelKey: "filter.text" },
  { key: "code", labelKey: "filter.code" },
  { key: "link", labelKey: "filter.link" },
  { key: "image", labelKey: "filter.image" },
  { key: "file", labelKey: "filter.file" },
];

async function toggleTrash() {
  const next = !clipboardStore.trashFilter;
  clipboardStore.setTrashFilter(next);
  if (next) {
    await clipboardStore.loadRecords();
  } else {
    await clipboardStore.search("");
  }
}

async function onEmptyTrash() {
  const ok = await confirm({
    title: t('confirm.emptyTrashTitle'),
    message: t('confirm.emptyTrashMsg'),
    confirmText: t('confirm.emptyTrashConfirm'),
    danger: true,
  });
  if (ok) {
    try {
      await clipboardStore.emptyTrash();
      toast(t('confirm.trashEmptied'), "success");
    } catch {
      toast(t('confirm.emptyFailed'), "error");
    }
  }
}
</script>

<style scoped>
.floating-panel {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-header {
  padding: var(--space-3) var(--space-4) var(--space-2);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  flex-shrink: 0;
}

.panel-title-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-2);
}

.panel-title {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--text-primary);
}

.header-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.filter-row {
  display: flex;
  gap: 2px;
  padding: 0 var(--space-4) var(--space-2);
  flex-shrink: 0;
  overflow-x: auto;
}

.filter-tab {
  height: var(--btn-height-sm);
  padding: 0 var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--text-md);
  font-weight: 500;
  color: var(--text-secondary);
  transition: all var(--transition-fast);
  display: flex;
  align-items: center;
  gap: var(--space-1);
  cursor: pointer;
}

.filter-tab:hover {
  background: var(--bg-hover);
  color: var(--accent-text);
}

.filter-tab.active {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.filter-count {
  font-size: var(--text-xs);
  background: var(--bg-active);
  padding: 1px var(--space-1);
  border-radius: 4px;
  color: var(--text-tertiary);
}

.filter-tab.active .filter-count {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.trash-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) var(--space-4) var(--space-3);
  font-size: var(--text-md);
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border-subtle);
}

.empty-trash-link {
  color: var(--danger);
  font-weight: 500;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.empty-trash-link:hover {
  background: var(--danger-soft);
}

.panel-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  position: relative;
}
</style>
