<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.data.title') }}</div>
    <div class="data-card">
      <div>
        <div class="setting-label">{{ $t('settings.data.exportTitle') }}</div>
        <div class="setting-desc">{{ $t('settings.data.exportDesc') }}</div>
      </div>
      <button class="btn btn-secondary" :disabled="isExporting" @click="exportData">
        <AppIcon v-if="!isExporting" name="package" :size="13" />
        {{ isExporting ? $t('settings.data.exporting') : $t('settings.data.exportBtn') }}
      </button>
    </div>
    <div v-if="exportStatus" class="status-line" :class="exportStatusKind">{{ exportStatus }}</div>

    <div class="data-card">
      <div>
        <div class="setting-label">{{ $t('settings.data.importTitle') }}</div>
        <div class="setting-desc">{{ $t('settings.data.importDesc') }}</div>
      </div>
      <button class="btn btn-secondary" :disabled="isImporting" @click="importData">
        <AppIcon v-if="!isImporting" name="history" :size="13" />
        {{ isImporting ? $t('settings.data.importing') : $t('settings.data.importBtn') }}
      </button>
    </div>
    <div v-if="importStatus" class="status-line" :class="importStatusKind">{{ importStatus }}</div>

    <template v-if="statsEnabled">
      <div class="settings-section-title" style="margin-top: 1.25rem">{{ $t('settings.data.storage') }}</div>
      <div class="data-card storage-card">
        <div class="storage-card-main">
          <div class="setting-label">{{ $t('settings.data.localStorage') }}</div>
          <div class="setting-desc">
            {{ $t('settings.data.storageDesc') }}
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
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useClipboardStore } from "../../stores/clipboard";
import { useFeature } from "../../composables/useFeature";
import AppIcon from "../icons/AppIcon.vue";

const clipboardStore = useClipboardStore();
const statsEnabled = useFeature("stats");
const { t } = useI18n();

const stats = computed(() => clipboardStore.stats);

const exportStatus = ref("");
const exportStatusKind = ref<"success" | "error" | "">("");
const importStatus = ref("");
const importStatusKind = ref<"success" | "error" | "">("");
const isExporting = ref(false);
const isImporting = ref(false);

async function exportData() {
  exportStatus.value = "";
  exportStatusKind.value = "";
  isExporting.value = true;
  try {
    const path = await save({
      defaultPath: `clipboard-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "Clipboard JSON", extensions: ["json"] }],
    });
    if (!path) return;
    // Backend streams JSON to disk — avoids holding the full export in JS/Rust heap.
    await invoke("export_data", { path });
    exportStatus.value = t('settings.data.exportDone');
    exportStatusKind.value = "success";
  } catch (e) {
    console.error("Export failed:", e);
    exportStatus.value = t('settings.data.exportFailed', { error: String(e) });
    exportStatusKind.value = "error";
  } finally {
    isExporting.value = false;
  }
}

async function importData() {
  importStatus.value = "";
  importStatusKind.value = "";
  isImporting.value = true;
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "Clipboard JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    const imported = await invoke<number>("import_data_from_path", { path });
    await clipboardStore.loadRecords();
    importStatus.value = t('settings.data.importDone', { count: imported });
    importStatusKind.value = "success";
  } catch (e) {
    console.error("Import failed:", e);
    importStatus.value = t('settings.data.importFailed', { error: String(e) });
    importStatusKind.value = "error";
  } finally {
    isImporting.value = false;
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<style scoped>
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
