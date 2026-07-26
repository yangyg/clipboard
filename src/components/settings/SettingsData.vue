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

    <div class="settings-section-title" style="margin-top: 1.25rem">{{ $t('settings.data.webdavTitle') }}</div>
    <p class="setting-desc" style="margin: 0 0 0.75rem">
      {{ $t('settings.data.webdavDesc') }}
    </p>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.data.webdavUrl') }}</span>
      <input
        class="auto-tag-input"
        type="url"
        placeholder="https://dav.jianguoyun.com/dav/"
        :value="settings.webdav_url"
        @input="update('webdav_url', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.data.webdavUsername') }}</span>
      <input
        class="auto-tag-input"
        type="text"
        autocomplete="username"
        :value="settings.webdav_username"
        @input="update('webdav_username', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.data.webdavPassword') }}</span>
      <input
        class="auto-tag-input"
        type="password"
        autocomplete="current-password"
        :value="settings.webdav_password"
        @input="update('webdav_password', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.data.webdavRemotePath') }}</span>
      <input
        class="auto-tag-input"
        type="text"
        placeholder="ClipVaultSync"
        :value="settings.webdav_remote_path"
        @input="update('webdav_remote_path', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.data.webdavSyncSensitive') }}</div>
        <div class="setting-desc">{{ $t('settings.data.webdavSyncSensitiveDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.webdav_sync_sensitive"
        :aria-label="$t('settings.data.webdavSyncSensitive')"
        @update:model-value="(v: boolean) => update('webdav_sync_sensitive', v)"
      />
    </div>
    <div class="data-card webdav-actions">
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavTest">
        <AppIcon name="cloud" :size="13" />
        {{ webdavAction === 'test' ? $t('settings.data.webdavTesting') : $t('settings.data.webdavTest') }}
      </button>
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPull">
        <AppIcon name="cloudDownload" :size="13" />
        {{ webdavAction === 'pull' ? $t('settings.data.webdavPulling') : $t('settings.data.webdavPull') }}
      </button>
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPush">
        <AppIcon name="cloudUpload" :size="13" />
        {{ webdavAction === 'push' ? $t('settings.data.webdavPushing') : $t('settings.data.webdavPush') }}
      </button>
      <button class="btn btn-primary" :disabled="webdavBusy" @click="webdavSyncNow">
        <AppIcon name="refresh" :size="13" />
        {{ webdavAction === 'sync' ? $t('settings.data.webdavSyncing') : $t('settings.data.webdavSync') }}
      </button>
    </div>
    <div v-if="settings.webdav_last_sync_at" class="setting-desc">
      {{ $t('settings.data.lastSync', { time: formatSyncTime(settings.webdav_last_sync_at) }) }}
    </div>
    <div v-if="webdavStatus" class="status-line" :class="webdavStatusKind">{{ webdavStatus }}</div>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.data.clearHistory') }}</div>
        <div class="setting-desc">{{ $t('settings.data.clearHistoryDesc') }}</div>
      </div>
      <button class="btn btn-danger" @click="clearHistory"><AppIcon name="trash" :size="13" /> {{ $t('settings.data.clearHistoryBtn') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import { useConfirm } from "../../composables/useConfirm";
import { useToast } from "../../composables/useToast";
import type { WebDavSyncResult } from "../../types";
import AppIcon from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();

const exportStatus = ref("");
const exportStatusKind = ref<"success" | "error" | "">("");
const importStatus = ref("");
const importStatusKind = ref<"success" | "error" | "">("");
const isExporting = ref(false);
const isImporting = ref(false);
const webdavBusy = ref(false);
const webdavAction = ref<"" | "test" | "pull" | "push" | "sync">("");
const webdavStatus = ref("");
const webdavStatusKind = ref<"success" | "error" | "">("");

async function exportData() {
  exportStatus.value = "";
  exportStatusKind.value = "";
  isExporting.value = true;
  try {
    const path = await save({
      defaultPath: `clipvault-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
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
      filters: [{ name: "ClipVault JSON", extensions: ["json"] }],
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

function formatSyncTime(iso: string) {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

async function flushSettings() {
  await settingsStore.saveSettings();
}

async function webdavTest() {
  webdavStatus.value = "";
  webdavStatusKind.value = "";
  webdavBusy.value = true;
  webdavAction.value = "test";
  try {
    await flushSettings();
    await invoke("webdav_test_connection");
    webdavStatus.value = t('settings.data.webdavConnected');
    webdavStatusKind.value = "success";
  } catch (e) {
    webdavStatus.value = t('settings.data.webdavConnectFailed', { error: String(e) });
    webdavStatusKind.value = "error";
  } finally {
    webdavBusy.value = false;
    webdavAction.value = "";
  }
}

async function runWebDav(
  action: "pull" | "push" | "sync",
  command: "webdav_pull" | "webdav_push" | "webdav_sync",
) {
  webdavStatus.value = "";
  webdavStatusKind.value = "";
  webdavBusy.value = true;
  webdavAction.value = action;
  try {
    await flushSettings();
    const result = await invoke<WebDavSyncResult>(command);
    webdavStatus.value = result.message;
    webdavStatusKind.value = "success";
    await settingsStore.loadSettings();
    if (action === "pull" || action === "sync") {
      await clipboardStore.loadRecords();
      await clipboardStore.loadStats();
    }
  } catch (e) {
    webdavStatus.value = `${action === "pull" ? t('settings.data.webdavPullFailed', { error: String(e) }) : action === "push" ? t('settings.data.webdavPushFailed', { error: String(e) }) : t('settings.data.webdavSyncFailed', { error: String(e) })}`;
    webdavStatusKind.value = "error";
  } finally {
    webdavBusy.value = false;
    webdavAction.value = "";
  }
}

async function webdavPull() {
  await runWebDav("pull", "webdav_pull");
}

async function webdavPush() {
  await runWebDav("push", "webdav_push");
}

async function webdavSyncNow() {
  await runWebDav("sync", "webdav_sync");
}

async function clearHistory() {
  const ok = await confirm({
    title: t('confirm.clearHistoryTitle'),
    message: t('confirm.clearHistoryMsg'),
    confirmText: t('confirm.clearHistoryConfirm'),
    cancelText: t('common.cancel'),
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke("clear_history");
    await clipboardStore.loadRecords();
    toast(t('confirm.historyCleared'), "success");
  } catch (e) {
    console.error("Clear history failed:", e);
    toast(t('confirm.clearHistoryFailed'), "error");
  }
}
</script>

<style scoped>
.webdav-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 10px;
}

.webdav-actions {
  flex-wrap: wrap;
  justify-content: flex-start;
}

.webdav-actions .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
</style>
