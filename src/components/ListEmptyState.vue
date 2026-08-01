<template>
  <!-- Loading (initial only) -->
  <div v-if="clipboardStore.isLoading && clipboardStore.records.length === 0" class="loading-state">
    <div class="loading-spinner"></div>
    <span>{{ $t('common.loading') }}</span>
  </div>

  <!-- Empty -->
  <div v-else-if="clipboardStore.filteredRecords.length === 0 && !clipboardStore.isLoading" class="empty-state">
    <div class="empty-icon"><AppIcon :name="emptyState.icon" :size="36" :stroke-width="1.5" /></div>
    <div class="empty-text">{{ emptyState.title }}</div>
    <div v-if="emptyState.hint" class="empty-hint">
      <template v-if="emptyState.clearSearch">
        {{ $t('emptyState.tryOtherKeywords') }}
        <button class="clear-link" @click="clipboardStore.search('')">{{ $t('common.clearSearch') }}</button>
      </template>
      <template v-else>{{ emptyState.hint }}</template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";

const clipboardStore = useClipboardStore();
const { t } = useI18n();

const TYPE_LABEL_KEYS: Record<string, string> = {
  text: 'filter.text',
  code: 'filter.code',
  link: 'filter.link',
  image: 'filter.image',
  file: 'filter.file',
};

const emptyState = computed(() => {
  if (clipboardStore.searchQuery) {
    return { icon: "search" as AppIconName, title: t('emptyState.noResults'), hint: "", clearSearch: true };
  }
  if (clipboardStore.trashFilter) {
    return { icon: "trash" as AppIconName, title: t('emptyState.trashEmpty'), hint: t('emptyState.trashHint'), clearSearch: false };
  }
  if (clipboardStore.activeTag && clipboardStore.activeFilter !== "all") {
    const typeLabel =
      clipboardStore.activeFilter === "favorites"
        ? t('filter.favorites')
        : t(TYPE_LABEL_KEYS[clipboardStore.activeFilter] ?? clipboardStore.activeFilter);
    return {
      icon: "tag" as AppIconName,
      title: t('emptyState.tagFilterEmpty', { type: typeLabel, tag: clipboardStore.activeTag }),
      hint: t('emptyState.tagFilterHint'),
      clearSearch: false,
    };
  }
  if (clipboardStore.activeTag) {
    return { icon: "tag" as AppIconName, title: t('emptyState.tagEmpty'), hint: t('emptyState.tagHint'), clearSearch: false };
  }
  if (clipboardStore.activeFilter === "favorites") {
    return { icon: "star" as AppIconName, title: t('emptyState.favoritesEmpty'), hint: t('emptyState.favoritesHint'), clearSearch: false };
  }
  if (clipboardStore.activeFilter !== "all") {
    const typeIconMap: Record<string, AppIconName> = {
      text: "type", code: "code", link: "link", image: "image", file: "file",
    };
    return {
      icon: typeIconMap[clipboardStore.activeFilter] ?? ("clipboard" as AppIconName),
      title: t('emptyState.typeEmpty', { type: t(TYPE_LABEL_KEYS[clipboardStore.activeFilter] ?? '') }),
      hint: t('emptyState.typeHint'),
      clearSearch: false,
    };
  }
  return { icon: "clipboard" as AppIconName, title: t('emptyState.allEmpty'), hint: t('emptyState.allHint'), clearSearch: false };
});
</script>

<style scoped>
.loading-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-md);
  flex: 1;
  padding: var(--space-5);
  text-align: center;
}

.empty-icon {
  width: 48px;
  height: 48px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-tertiary);
  opacity: 0.9;
  margin-bottom: 4px;
}

.empty-text {
  font-size: var(--text-base);
}

.empty-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.clear-link {
  color: var(--accent-text);
  cursor: pointer;
  text-decoration: underline;
}
</style>
