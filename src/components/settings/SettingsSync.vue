<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.sync.title') }}</div>
    <p class="setting-desc" style="margin: 0 0 0.75rem">
      {{ $t('settings.sync.webdavDesc') }}
    </p>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavUrl') }}</span>
      <input
        class="auto-tag-input"
        type="url"
        placeholder="https://dav.jianguoyun.com/dav/"
        :value="settings.webdav_url"
        @input="update('webdav_url', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavUsername') }}</span>
      <input
        class="auto-tag-input"
        type="text"
        autocomplete="username"
        :value="settings.webdav_username"
        @input="update('webdav_username', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavPassword') }}</span>
      <input
        class="auto-tag-input"
        type="password"
        autocomplete="current-password"
        :value="settings.webdav_password"
        @input="update('webdav_password', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label class="webdav-field">
      <span class="setting-label">{{ $t('settings.sync.webdavRemotePath') }}</span>
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
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import type { WebDavSyncResult } from "../../types";
import AppIcon from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const clipboardStore = useClipboardStore();
const { t } = useI18n();

const webdavBusy = ref(false);
const webdavAction = ref<"" | "test" | "pull" | "push" | "sync">("");
const webdavStatus = ref("");
const webdavStatusKind = ref<"success" | "error" | "">("");

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
    webdavStatus.value = result.message;
    webdavStatusKind.value = "success";
    await settingsStore.loadSettings();
    if (action === "pull" || action === "sync") {
      await clipboardStore.loadRecords();
      await clipboardStore.loadStats();
    }
  } catch (e) {
    webdavStatus.value = `${action === "pull" ? t('settings.sync.webdavPullFailed', { error: String(e) }) : action === "push" ? t('settings.sync.webdavPushFailed', { error: String(e) }) : t('settings.sync.webdavSyncFailed', { error: String(e) })}`;
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
