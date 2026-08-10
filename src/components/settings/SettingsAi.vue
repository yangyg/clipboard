<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.ai.title') }}</div>
    <p class="setting-desc" style="margin: 0 0 0.75rem">
      {{ $t('settings.ai.aiDesc') }}
    </p>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.ai.enable') }}</div>
        <div class="setting-desc">{{ $t('settings.ai.enableDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.enable_ai"
        :aria-label="$t('settings.ai.enable')"
        @update:model-value="(v: boolean) => update('enable_ai', v)"
      />
    </div>

    <template v-if="settings.enable_ai">
      <div class="ai-field">
        <span class="setting-label">{{ $t('settings.ai.provider') }}</span>
        <div class="ai-preset-chips" role="group" :aria-label="$t('settings.ai.provider')">
          <button
            v-for="preset in PRESETS"
            :key="preset.key"
            type="button"
            class="ai-preset-chip"
            :class="{ active: settings.ai_base_url === preset.baseUrl }"
            :aria-pressed="settings.ai_base_url === preset.baseUrl"
            @click="applyPreset(preset)"
          >
            <AppIcon :name="preset.icon" :size="12" />
            {{ $t(preset.labelKey) }}
          </button>
        </div>
      </div>

      <label class="ai-field">
        <span class="setting-label">{{ $t('settings.ai.baseUrl') }}</span>
        <TextInput
          class="auto-tag-input"
          type="url"
          :model-value="settings.ai_base_url"
          :placeholder="'https://api.openai.com/v1'"
          @update:model-value="(v) => update('ai_base_url', v)"
        />
      </label>

      <label class="ai-field">
        <span class="setting-label">{{ $t('settings.ai.apiKey') }}</span>
        <PasswordInput
          class="auto-tag-input"
          :model-value="settings.ai_api_key"
          :placeholder="settings.ai_api_key ? '••••••••' : ''"
          @update:model-value="(v) => update('ai_api_key', v)"
        />
      </label>

      <label class="ai-field">
        <span class="setting-label">{{ $t('settings.ai.model') }}</span>
        <TextInput
          class="auto-tag-input ai-model-input"
          type="text"
          :model-value="settings.ai_model"
          :placeholder="'gpt-4o-mini'"
          @update:model-value="(v) => update('ai_model', v)"
        />
      </label>

      <div class="setting-row">
        <div>
          <div class="setting-label">{{ $t('settings.ai.summaryAlias') }}</div>
          <div class="setting-desc">{{ $t('settings.ai.summaryAliasDesc') }}</div>
        </div>
        <ToggleSwitch
          :model-value="settings.ai_summary_alias"
          :aria-label="$t('settings.ai.summaryAlias')"
          @update:model-value="(v: boolean) => update('ai_summary_alias', v)"
        />
      </div>

      <div class="setting-row">
        <div>
          <div class="setting-label">{{ $t('settings.ai.autoTag') }}</div>
          <div class="setting-desc">{{ $t('settings.ai.autoTagDesc') }}</div>
        </div>
        <ToggleSwitch
          :model-value="settings.ai_auto_tag"
          :aria-label="$t('settings.ai.autoTag')"
          @update:model-value="(v: boolean) => update('ai_auto_tag', v)"
        />
      </div>

      <div class="setting-row">
        <div>
          <div class="setting-label">{{ $t('settings.ai.maxChars') }}</div>
          <div class="setting-desc">{{ $t('settings.ai.maxCharsDesc') }}</div>
        </div>
        <input
          class="ai-chars-input"
          type="number"
          min="64"
          max="20000"
          step="100"
          :value="settings.ai_max_chars"
          :aria-label="$t('settings.ai.maxChars')"
          @change="updateAiMaxChars(($event.target as HTMLInputElement).value)"
        />
      </div>

      <div class="data-card ai-actions">
        <button type="button" class="btn btn-secondary" :disabled="aiBusy" @click="aiTest">
          <AppIcon name="sparkles" :size="13" />
          {{ aiBusy ? $t('settings.ai.testing') : $t('settings.ai.test') }}
        </button>
      </div>
      <div v-if="aiStatus" class="status-line" :class="aiStatusKind">{{ aiStatus }}</div>
      <p class="setting-desc ai-privacy-note">{{ $t('settings.ai.privacyNote') }}</p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../composables/useSettings";
import AppIcon, { type AppIconName } from "../icons/AppIcon.vue";
import PasswordInput from "../PasswordInput.vue";
import TextInput from "../TextInput.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const { t } = useI18n();

const aiBusy = ref(false);
const aiStatus = ref("");
const aiStatusKind = ref<"success" | "error" | "">("");

const PRESETS: {
  key: string;
  labelKey: string;
  icon: AppIconName;
  baseUrl: string;
  model: string;
}[] = [
  {
    key: "openai",
    labelKey: "settings.ai.presetOpenai",
    icon: "sparkles",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
  },
  {
    key: "deepseek",
    labelKey: "settings.ai.presetDeepseek",
    icon: "zap",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  {
    key: "moonshot",
    labelKey: "settings.ai.presetMoonshot",
    icon: "cloud",
    baseUrl: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
  },
  {
    key: "qwen",
    labelKey: "settings.ai.presetQwen",
    icon: "cloudUpload",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-plus",
  },
  {
    key: "ollama",
    labelKey: "settings.ai.presetOllama",
    icon: "monitor",
    baseUrl: "http://localhost:11434/v1",
    model: "llama3",
  },
];

function applyPreset(preset: (typeof PRESETS)[number]) {
  update("ai_base_url", preset.baseUrl);
  update("ai_model", preset.model);
}

function updateAiMaxChars(value: string) {
  const n = Number.parseInt(value, 10);
  if (Number.isNaN(n)) return;
  update("ai_max_chars", Math.min(20000, Math.max(64, n)));
}

async function aiTest() {
  aiStatus.value = "";
  aiStatusKind.value = "";
  aiBusy.value = true;
  try {
    await settingsStore.saveSettings();
    await invoke("ai_test_connection");
    aiStatus.value = t("settings.ai.connected");
    aiStatusKind.value = "success";
  } catch (e) {
    aiStatus.value = t("settings.ai.connectFailed", { error: String(e) });
    aiStatusKind.value = "error";
  } finally {
    aiBusy.value = false;
  }
}
</script>

<style scoped>
.ai-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 10px;
}

.ai-preset-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.ai-preset-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-pill);
  background: var(--bg-input);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition:
    background var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.ai-preset-chip:hover {
  border-color: var(--border-default);
  color: var(--text-primary);
}

.ai-preset-chip.active {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  color: var(--accent-text);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 20%, transparent);
}

.ai-model-input {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.ai-chars-input {
  width: 96px;
  height: var(--btn-height-lg);
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: var(--text-md);
  font-variant-numeric: tabular-nums;
}

.ai-actions {
  flex-wrap: wrap;
  justify-content: flex-start;
}

.ai-actions .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.ai-privacy-note {
  margin: 4px 0 0;
  line-height: 1.5;
}
</style>