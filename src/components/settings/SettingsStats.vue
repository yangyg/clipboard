<template>
  <div class="stats-dashboard">
    <div class="stats-card">
      <div class="stats-value accent">{{ stats?.total_records ?? 0 }}</div>
      <div class="stats-label">{{ $t('settings.stats.totalRecords') }}</div>
    </div>
    <div class="stats-card">
      <div class="stats-value success">{{ stats?.total_copies ?? 0 }}</div>
      <div class="stats-label">{{ $t('settings.stats.totalCopies') }}</div>
    </div>
    <div class="stats-card">
      <div class="stats-value warning">{{ stats?.favorites_count ?? 0 }}</div>
      <div class="stats-label">{{ $t('settings.stats.favorites') }}</div>
    </div>
    <div class="stats-card">
      <div class="stats-value sensitive">{{ stats?.sensitive_count ?? 0 }}</div>
      <div class="stats-label">{{ $t('settings.stats.sensitive') }}</div>
    </div>
  </div>

  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.stats.typeDistribution') }}</div>
    <div class="type-bars">
      <div v-for="item in typeDistribution" :key="item.key" class="type-row">
        <div class="type-row-label">
          <span>{{ item.label }}</span>
          <span>{{ item.count }}</span>
        </div>
        <div class="type-track">
          <div class="type-fill" :style="{ width: item.percent + '%' }"></div>
        </div>
      </div>
    </div>
  </div>

  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.stats.storage') }}</div>
    <div class="data-card storage-card">
      <div class="storage-card-main">
        <div class="setting-label">{{ $t('settings.stats.localStorage') }}</div>
        <div class="setting-desc">
          {{ $t('settings.stats.storageDesc') }}
        </div>
        <div
          v-if="stats?.data_path"
          class="storage-path"
          :title="stats.data_path"
        >
          {{ stats.data_path }}
        </div>
      </div>
      <span class="kbd-display">{{ formatBytes(stats?.storage_bytes ?? 0) }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../../stores/clipboard";

const clipboardStore = useClipboardStore();
const { t } = useI18n();

const stats = computed(() => clipboardStore.stats);

const TYPE_LABELS: Record<string, string> = {
  text: "settings.tags.typeText",
  code: "settings.tags.typeCode",
  link: "settings.tags.typeLink",
  image: "settings.tags.typeImage",
  file: "settings.tags.typeFile",
  sensitive: "settings.stats.sensitive",
};

const typeDistribution = computed(() => {
  const distribution = stats.value?.type_distribution ?? {};
  const total = Math.max(stats.value?.total_records ?? 0, 1);
  return Object.entries(distribution).map(([key, rawCount]) => {
    const count = Number(rawCount) || 0;
    return {
      key,
      count,
      label: TYPE_LABELS[key] ? t(TYPE_LABELS[key]) : key,
      percent: Math.max(4, Math.round((count / total) * 100)),
    };
  });
});

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<style scoped>
.stats-dashboard {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 22px;
}

.stats-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 14px;
}

.stats-value {
  font-family: var(--font-mono);
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1;
}

.stats-value.accent { color: var(--accent); }
.stats-value.success { color: var(--success); }
.stats-value.warning { color: var(--warning); }
.stats-value.sensitive { color: var(--sensitive); }

.stats-label {
  margin-top: 6px;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.type-bars {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.type-row-label {
  display: flex;
  justify-content: space-between;
  margin-bottom: 5px;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.type-track {
  height: 6px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--bg-active);
}

.type-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}

.storage-card {
  align-items: flex-start;
}

.storage-card-main {
  min-width: 0;
  flex: 1;
}

.storage-path {
  margin-top: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-active);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  line-height: 1.4;
  word-break: break-all;
  user-select: text;
}
</style>
