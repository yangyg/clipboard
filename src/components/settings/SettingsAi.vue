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

      <div class="ai-field">
        <span class="setting-label">{{ $t('settings.ai.model') }}</span>
        <p class="setting-desc ai-model-desc">{{ $t('settings.ai.modelDesc') }}</p>
        <div
          class="ai-model-list"
          role="radiogroup"
          :aria-label="$t('settings.ai.model')"
        >
          <div
            v-for="(name, index) in settings.ai_models"
            :key="index"
            class="ai-model-item"
            :class="{ active: name === settings.ai_model }"
          >
            <button
              type="button"
              class="ai-model-radio"
              role="radio"
              :aria-checked="name === settings.ai_model"
              :aria-label="$t('settings.ai.modelCurrent')"
              @click="update('ai_model', name)"
            />
            <TextInput
              class="auto-tag-input ai-model-input"
              type="text"
              :model-value="name"
              :placeholder="'gpt-4o-mini'"
              :aria-label="$t('settings.ai.model')"
              @change="renameModel(index, ($event.target as HTMLInputElement).value)"
            />
            <button
              type="button"
              class="ai-model-remove"
              :disabled="settings.ai_models.length <= 1"
              :title="settings.ai_models.length <= 1 ? $t('settings.ai.modelKeepOne') : $t('settings.ai.removeModel', { name })"
              :aria-label="$t('settings.ai.removeModel', { name })"
              @click="removeModel(index)"
            >
              <AppIcon name="close" :size="12" />
            </button>
          </div>
        </div>
        <div class="ai-model-add-row">
          <TextInput
            ref="addInput"
            class="auto-tag-input ai-model-input ai-model-add-input"
            type="text"
            v-model="newModel"
            :placeholder="'gpt-4o-mini'"
            :aria-label="$t('settings.ai.addModel')"
            @keydown.enter="addModel"
          />
          <button
            type="button"
            class="btn btn-primary btn-lg ai-model-add-btn"
            :disabled="addDisabled"
            @click="addModel"
          >
            <AppIcon name="plus" :size="13" /> {{ $t('settings.ai.addModel') }}
          </button>
        </div>
      </div>

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
          <div class="setting-label">{{ $t('settings.ai.minChars') }}</div>
          <div class="setting-desc">{{ $t('settings.ai.minCharsDesc') }}</div>
        </div>
        <input
          class="ai-chars-input"
          type="number"
          min="0"
          :max="settings.ai_max_chars"
          step="10"
          :value="settings.ai_min_chars"
          :aria-label="$t('settings.ai.minChars')"
          @change="updateAiMinChars(($event.target as HTMLInputElement).value)"
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
          :min="settings.ai_min_chars"
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
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../composables/useSettings";
import { useToast } from "../../composables/useToast";
import { AI_MODELS_MAX } from "../../utils/aiModels";
import AppIcon, { type AppIconName } from "../icons/AppIcon.vue";
import PasswordInput from "../PasswordInput.vue";
import TextInput from "../TextInput.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const { t } = useI18n();
const { toast } = useToast();

const aiBusy = ref(false);
const aiStatus = ref("");
const aiStatusKind = ref<"success" | "error" | "">("");
const newModel = ref("");
const addInput = ref<InstanceType<typeof TextInput> | null>(null);

const addDisabled = computed(() => newModel.value.trim() === "");

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
  if (!settings.ai_models.includes(preset.model)) {
    if (settings.ai_models.length >= AI_MODELS_MAX) {
      toast(t("settings.ai.modelMax"), "warning");
      return;
    }
    update("ai_models", [...settings.ai_models, preset.model]);
  }
  update("ai_model", preset.model);
}

function addModel() {
  const name = newModel.value.trim();
  if (!name) {
    toast(t("settings.ai.modelEmpty"), "warning");
    return;
  }
  if (settings.ai_models.includes(name)) {
    toast(t("settings.ai.modelDuplicate"), "warning");
    return;
  }
  if (settings.ai_models.length >= AI_MODELS_MAX) {
    toast(t("settings.ai.modelMax"), "warning");
    return;
  }
  update("ai_models", [...settings.ai_models, name]);
  newModel.value = "";
  addInput.value?.focus();
}

function renameModel(index: number, nextRaw: string) {
  const next = nextRaw.trim();
  const prev = settings.ai_models[index];
  if (!prev || !next || next === prev) return;
  if (settings.ai_models.some((m, i) => i !== index && m === next)) {
    toast(t("settings.ai.modelDuplicate"), "warning");
    return;
  }
  update(
    "ai_models",
    settings.ai_models.map((m, i) => (i === index ? next : m)),
  );
  if (settings.ai_model === prev) update("ai_model", next);
}

function removeModel(index: number) {
  if (settings.ai_models.length <= 1) return;
  const removed = settings.ai_models[index];
  const list = settings.ai_models.filter((_, i) => i !== index);
  update("ai_models", list);
  if (settings.ai_model === removed) update("ai_model", list[0]);
}

function updateAiMinChars(value: string) {
  const n = Number.parseInt(value, 10);
  if (Number.isNaN(n)) return;
  update("ai_min_chars", Math.max(0, Math.min(settings.ai_max_chars, n)));
}

function updateAiMaxChars(value: string) {
  const n = Number.parseInt(value, 10);
  if (Number.isNaN(n)) return;
  update("ai_max_chars", Math.min(20000, Math.max(settings.ai_min_chars, n)));
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

.ai-model-desc {
  margin: 0;
}

.ai-model-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ai-model-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.ai-model-item:hover {
  background: var(--accent-softer);
}

.ai-model-item.active {
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.ai-model-radio {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  padding: 0;
  border: 1px solid var(--border-default);
  border-radius: 50%;
  background: var(--bg-input);
  cursor: pointer;
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast),
    background var(--transition-fast);
}

.ai-model-radio[aria-checked="true"] {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 4px var(--accent);
  background: var(--bg-surface);
}

.ai-model-input {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.ai-model-item :deep(.input-shell) {
  flex: 1;
  min-width: 0;
}

.ai-model-remove {
  flex-shrink: 0;
  font-size: var(--text-md);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: var(--radius-xs);
  transition:
    background var(--transition-fast),
    color var(--transition-fast);
}

.ai-model-remove:hover:not(:disabled) {
  background: var(--danger-soft);
  color: var(--danger);
}

.ai-model-remove:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.ai-model-add-row {
  display: flex;
  gap: var(--space-2);
  margin-top: 4px;
}

.ai-model-add-row :deep(.input-shell) {
  flex: 1;
  min-width: 0;
}

.ai-model-add-btn {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
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