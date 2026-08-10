<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.sync.title') }}</div>
    <p class="setting-desc" style="margin: 0 0 0.75rem">
      {{ $t('settings.sync.webdavDesc') }}
    </p>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavUrl') }}</span>
      <TextInput
        class="auto-tag-input"
        type="url"
        placeholder="https://dav.jianguoyun.com/dav/"
        :model-value="settings.webdav_url"
        @update:model-value="(v) => update('webdav_url', v)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavUsername') }}</span>
      <TextInput
        class="auto-tag-input"
        type="text"
        autocomplete="username"
        :model-value="settings.webdav_username"
        @update:model-value="(v) => update('webdav_username', v)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavPassword') }}</span>
      <PasswordInput
        class="auto-tag-input"
        autocomplete="current-password"
        :model-value="settings.webdav_password"
        @update:model-value="(v) => update('webdav_password', v)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavRemotePath') }}</span>
      <TextInput
        class="auto-tag-input"
        type="text"
        placeholder="ClipVaultSync"
        :model-value="settings.webdav_remote_path"
        @update:model-value="(v) => update('webdav_remote_path', v)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavDeviceName') }}</span>
      <div class="setting-desc">{{ $t('settings.sync.webdavDeviceNameDesc') }}</div>
      <TextInput
        class="auto-tag-input"
        type="text"
        :model-value="settings.webdav_device_name"
        @update:model-value="(v) => update('webdav_device_name', v)"
      />
    </label>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.sync.webdavSyncSensitive') }}</div>
        <div class="setting-desc">{{ $t('settings.sync.webdavSyncSensitiveDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.webdav_sync_sensitive"
        :aria-label="$t('settings.sync.webdavSyncSensitive')"
        @update:model-value="(v: boolean) => update('webdav_sync_sensitive', v)"
      />
    </div>
    <div class="data-card webdav-actions">
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavTest">
        <AppIcon name="cloud" :size="13" />
        {{ webdavAction === 'test' ? $t('settings.sync.webdavTesting') : $t('settings.sync.webdavTest') }}
      </button>
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPull">
        <AppIcon name="cloudDownload" :size="13" />
        {{ webdavAction === 'pull' ? $t('settings.sync.webdavPulling') : $t('settings.sync.webdavPull') }}
      </button>
      <button class="btn btn-secondary" :disabled="webdavBusy" @click="webdavPush">
        <AppIcon name="cloudUpload" :size="13" />
        {{ webdavAction === 'push' ? $t('settings.sync.webdavPushing') : $t('settings.sync.webdavPush') }}
      </button>
      <button class="btn btn-primary" :disabled="webdavBusy" @click="webdavSyncNow">
        <AppIcon name="refresh" :size="13" />
        {{ webdavAction === 'sync' ? $t('settings.sync.webdavSyncing') : $t('settings.sync.webdavSync') }}
      </button>
    </div>
    <div v-if="settings.webdav_last_sync_at" class="setting-desc">
      {{ $t('settings.sync.lastSync', { time: formatSyncTime(settings.webdav_last_sync_at) }) }}
    </div>
    <div v-if="webdavStatus" class="status-line" :class="webdavStatusKind">{{ webdavStatus }}</div>

    <div class="data-card sync-history">
      <div class="sync-history-head">
        <h3 class="sync-history-title">{{ $t('settings.sync.historyTitle') }}</h3>
        <button
          class="btn btn-secondary sync-history-clear"
          :disabled="historyBusy || syncHistory.length === 0"
          @click="clearHistory"
        >
          {{ $t('settings.sync.historyClear') }}
        </button>
      </div>
      <p v-if="syncHistory.length === 0" class="setting-desc sync-history-empty">
        {{ $t('settings.sync.historyEmpty') }}
      </p>
      <div v-else class="sync-history-scroll">
        <table class="sync-history-table">
          <thead>
            <tr>
              <th class="col-time">{{ $t('settings.sync.historyColTime') }}</th>
              <th>{{ $t('settings.sync.historyColAction') }}</th>
              <th>{{ $t('settings.sync.historyColStatus') }}</th>
              <th>{{ $t('settings.sync.historyColContent') }}</th>
              <th class="col-num">{{ $t('settings.sync.historyColTags') }}</th>
              <th>{{ $t('settings.sync.historyColMedia') }}</th>
              <th class="col-error">{{ $t('settings.sync.historyColError') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="h in syncHistory" :key="h.id">
              <td class="col-time">{{ formatSyncTime(h.synced_at) }}</td>
              <td>
                <span class="sync-history-action">{{ actionLabel(h.action) }}</span>
              </td>
              <td>
                <span class="sync-history-status" :class="h.success ? 'ok' : 'err'">
                  {{ h.success ? $t('settings.sync.historySuccess') : $t('settings.sync.historyFailed') }}
                </span>
              </td>
              <td class="sync-history-content">{{ h.success ? contentSummary(h) : '—' }}</td>
              <td class="col-num">{{ tagCount(h) > 0 ? tagCount(h) : '—' }}</td>
              <td class="sync-history-media">{{ h.success ? mediaSummaryCell(h) || '—' : '—' }}</td>
              <td class="col-error">
                <span v-if="!h.success && h.error" class="sync-history-error" :title="h.error">
                  {{ truncateError(h.error) }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import type { SyncHistoryEntry, WebDavSyncResult } from "../../types";
import { formatWebDavResult } from "../../utils/webdavResult";
import AppIcon from "../icons/AppIcon.vue";
import PasswordInput from "../PasswordInput.vue";
import TextInput from "../TextInput.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const clipboardStore = useClipboardStore();
const { t } = useI18n();

const webdavBusy = ref(false);
const webdavAction = ref<"" | "test" | "pull" | "push" | "sync">("");
const webdavStatus = ref("");
const webdavStatusKind = ref<"success" | "error" | "">("");

const syncHistory = ref<SyncHistoryEntry[]>([]);
const historyBusy = ref(false);

function formatSyncTime(iso: string) {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

function actionLabel(action: string) {
  if (action === "pull") return t('settings.sync.historyActionPull');
  if (action === "push") return t('settings.sync.historyActionPush');
  return t('settings.sync.historyActionSync');
}

function contentSummary(h: SyncHistoryEntry): string {
  const parts: string[] = [];
  if (h.pulled > 0) parts.push(t('settings.sync.historyNew', { count: h.pulled }));
  if (h.merged > 0) parts.push(t('settings.sync.historyMerged', { count: h.merged }));
  if (h.pushed > 0) parts.push(t('settings.sync.historyPushed', { count: h.pushed }));
  return parts.join(" · ");
}

function mediaSummaryCell(h: SyncHistoryEntry): string {
  const parts: string[] = [];
  if (h.media_downloaded > 0) {
    parts.push(t('settings.sync.historyMediaDown', { count: h.media_downloaded }));
  }
  if (h.media_uploaded > 0) {
    parts.push(t('settings.sync.historyMediaUp', { count: h.media_uploaded }));
  }
  if (h.media_skipped > 0) {
    parts.push(t('settings.sync.historyMediaSkip', { count: h.media_skipped }));
  }
  return parts.join(" · ");
}

function tagCount(h: SyncHistoryEntry): number {
  if (h.action === "pull") return h.tags_pulled;
  if (h.action === "push") return h.tags_pushed;
  return h.tags_pulled + h.tags_pushed;
}

function truncateError(err: string) {
  return err.length > 80 ? `${err.slice(0, 80)}…` : err;
}

async function loadHistory() {
  historyBusy.value = true;
  try {
    syncHistory.value = await invoke<SyncHistoryEntry[]>("get_sync_history", { limit: 20 });
  } catch (e) {
    console.error("Failed to load sync history:", e);
  } finally {
    historyBusy.value = false;
  }
}

async function clearHistory() {
  historyBusy.value = true;
  try {
    await invoke("clear_sync_history");
    syncHistory.value = [];
  } catch (e) {
    console.error("Failed to clear sync history:", e);
  } finally {
    historyBusy.value = false;
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
    webdavStatus.value = t('settings.sync.webdavConnected');
    webdavStatusKind.value = "success";
  } catch (e) {
    webdavStatus.value = t('settings.sync.webdavConnectFailed', { error: String(e) });
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
    webdavStatus.value = formatWebDavResult(result, action, t);
    webdavStatusKind.value = "success";
    await settingsStore.loadSettings();
    if (action === "pull" || action === "sync") {
      await clipboardStore.loadRecords();
      await clipboardStore.loadStats();
    }
    await loadHistory();
  } catch (e) {
    webdavStatus.value = `${action === "pull" ? t('settings.sync.webdavPullFailed', { error: String(e) }) : action === "push" ? t('settings.sync.webdavPushFailed', { error: String(e) }) : t('settings.sync.webdavSyncFailed', { error: String(e) })}`;
    webdavStatusKind.value = "error";
    await loadHistory();
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

onMounted(() => {
  void loadHistory();
});
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

.data-card.sync-history {
  margin-top: 12px;
  flex-direction: column;
  align-items: stretch;
  gap: 0;
  padding: 12px;
}

.sync-history-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.sync-history-title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: 600;
}

.sync-history-clear {
  padding: 3px 10px;
  font-size: var(--text-sm);
}

.sync-history-empty {
  margin: 0;
}

.sync-history-scroll {
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
  border-radius: var(--sketch-radius, 10px);
}

.sync-history-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm, 0.75rem);
}

.sync-history-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  text-align: left;
  font-weight: 600;
  padding: 8px 10px;
  color: var(--text-secondary, inherit);
  background: var(--bg-hover, rgba(128, 128, 128, 0.15));
  border-bottom: 1px solid var(--border-default, rgba(255, 255, 255, 0.1));
  white-space: nowrap;
}

.sync-history-table td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
  vertical-align: top;
  white-space: nowrap;
}

.sync-history-table tbody tr:last-child td {
  border-bottom: none;
}

.sync-history-table .col-time {
  color: var(--text-tertiary, inherit);
}

.sync-history-table .col-num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.sync-history-table .col-error {
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sync-history-action {
  font-weight: 600;
  padding: 1px 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-hover, rgba(128, 128, 128, 0.15));
}

.sync-history-status.ok {
  color: var(--success);
}

.sync-history-status.err {
  color: var(--danger);
}

.sync-history-content,
.sync-history-media {
  color: var(--text-secondary, inherit);
}

.sync-history-error {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--danger);
}
</style>
