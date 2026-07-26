<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.tags.title') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.tags.autoTag') }}</div>
        <div class="setting-desc">{{ $t('settings.tags.autoTagDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.enable_auto_tag"
        :aria-label="$t('settings.tags.autoTag')"
        @update:model-value="(v: boolean) => update('enable_auto_tag', v)"
      />
    </div>

    <div v-if="settings.enable_auto_tag" class="auto-tag-panel">
      <div class="auto-tag-panel-head">
        <div class="auto-tag-panel-title">{{ $t('settings.tags.matchRules') }}</div>
        <div class="auto-tag-panel-meta">{{ $t('settings.tags.rulesCount', { count: rulesDraft.length }) }}</div>
      </div>

      <div v-if="rulesDraft.length === 0" class="auto-tag-empty">
        <AppIcon name="tag" :size="18" />
        <p>{{ $t('settings.tags.noRules') }}</p>
      </div>

      <div v-else class="auto-tag-rules">
        <article
          v-for="(rule, index) in rulesDraft"
          :key="index"
          class="auto-tag-rule"
        >
          <header class="auto-tag-rule-top">
            <span
              class="auto-tag-rule-dot"
              :style="{ background: ruleAccentColor(rule.tag_name, index) }"
              aria-hidden="true"
            ></span>
            <span class="auto-tag-rule-index">{{ $t('settings.tags.rule', { index: index + 1 }) }}</span>
            <button
              type="button"
              class="auto-tag-remove"
              :title="$t('settings.tags.deleteRule')"
              :aria-label="$t('settings.tags.deleteRule')"
              @click="removeAutoTagRule(index)"
            >
              <AppIcon name="close" :size="12" />
            </button>
          </header>

          <label class="auto-tag-field">
            <span class="auto-tag-field-label">{{ $t('settings.tags.tagName') }}</span>
            <input
              class="auto-tag-input"
              :value="rule.tag_name"
              :placeholder="$t('settings.tags.tagNamePlaceholder')"
              @input="updateRuleField(index, 'tag_name', (($event.target as HTMLInputElement).value))"
            />
          </label>

          <label class="auto-tag-field">
            <span class="auto-tag-field-label">{{ $t('settings.tags.keywords') }}</span>
            <input
              class="auto-tag-input auto-tag-input-mono"
              :value="rule.keywords.join(', ')"
              :placeholder="$t('settings.tags.keywordsPlaceholder')"
              @change="updateRuleKeywords(index, ($event.target as HTMLInputElement).value)"
            />
            <div v-if="rule.keywords.length" class="auto-tag-keyword-chips" aria-hidden="true">
              <span
                v-for="kw in rule.keywords"
                :key="kw"
                class="auto-tag-chip auto-tag-chip-kw"
              >{{ kw }}</span>
            </div>
          </label>

          <div class="auto-tag-field">
            <span class="auto-tag-field-label">{{ $t('settings.tags.contentTypes') }}</span>
            <div class="auto-tag-type-chips" role="group" :aria-label="$t('settings.tags.contentTypes')">
              <button
                v-for="ct in CONTENT_TYPE_OPTIONS"
                :key="ct.value"
                type="button"
                class="auto-tag-type-chip"
                :class="{ active: rule.content_types.includes(ct.value) }"
                :style="rule.content_types.includes(ct.value) ? { '--chip-accent': ct.color } : undefined"
                :aria-pressed="rule.content_types.includes(ct.value)"
                @click="toggleRuleContentType(index, ct.value)"
              >
                <AppIcon :name="ct.icon" :size="12" />
                {{ $t(ct.labelKey) }}
              </button>
            </div>
          </div>
        </article>
      </div>

      <div class="auto-tag-actions">
        <button type="button" class="btn btn-secondary" @click="addAutoTagRule">
          <AppIcon name="plus" :size="13" /> {{ $t('settings.tags.addRule') }}
        </button>
        <button type="button" class="btn btn-secondary" @click="restoreDefaultAutoTagRules">
          <AppIcon name="restore" :size="13" /> {{ $t('settings.tags.restoreDefaults') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onUnmounted, ref, watch } from "vue";
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import { DEFAULT_AUTO_TAG_RULES, type AutoTagRule } from "../../types";
import { resolveKnownTagColors, resolveTagPalette } from "../../utils/themeColors";
import AppIcon, { type AppIconName } from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();
const clipboardStore = useClipboardStore();

const CONTENT_TYPE_OPTIONS = [
  { value: "text", labelKey: "settings.tags.typeText", icon: "type" as AppIconName, color: "var(--type-text)" },
  { value: "code", labelKey: "settings.tags.typeCode", icon: "code" as AppIconName, color: "var(--type-code)" },
  { value: "link", labelKey: "settings.tags.typeLink", icon: "link" as AppIconName, color: "var(--type-link)" },
  { value: "image", labelKey: "settings.tags.typeImage", icon: "image" as AppIconName, color: "var(--type-image)" },
  { value: "file", labelKey: "settings.tags.typeFile", icon: "file" as AppIconName, color: "var(--type-file)" },
] as const;

function cloneRules(rules: AutoTagRule[]): AutoTagRule[] {
  return rules.map((r) => ({
    tag_name: r.tag_name,
    keywords: [...r.keywords],
    content_types: [...r.content_types],
  }));
}

/** Local draft so typing rules doesn't deep-watch/save settings on every keystroke. */
const rulesDraft = ref<AutoTagRule[]>(cloneRules(settings.auto_tag_rules));
let rulesCommitTimer: ReturnType<typeof setTimeout> | null = null;
let ignoreRulesSettingsEcho = false;

watch(
  () => settings.auto_tag_rules,
  (rules) => {
    if (ignoreRulesSettingsEcho) return;
    rulesDraft.value = cloneRules(rules);
  },
  { deep: true },
);

function flushAutoTagRules() {
  if (rulesCommitTimer) {
    clearTimeout(rulesCommitTimer);
    rulesCommitTimer = null;
  }
  ignoreRulesSettingsEcho = true;
  update("auto_tag_rules", cloneRules(rulesDraft.value));
  void nextTick(() => {
    ignoreRulesSettingsEcho = false;
  });
}

function scheduleCommitRules() {
  if (rulesCommitTimer) clearTimeout(rulesCommitTimer);
  rulesCommitTimer = setTimeout(() => {
    rulesCommitTimer = null;
    flushAutoTagRules();
  }, 400);
}

function updateRuleField(index: number, field: "tag_name", value: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  next[index][field] = value;
  rulesDraft.value = next;
  scheduleCommitRules();
}

function updateRuleKeywords(index: number, raw: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  next[index].keywords = raw
    .split(/[,，]/)
    .map((s) => s.trim())
    .filter(Boolean);
  rulesDraft.value = next;
  scheduleCommitRules();
}

function toggleRuleContentType(index: number, contentType: string) {
  const next = cloneRules(rulesDraft.value);
  if (!next[index]) return;
  const types = next[index].content_types;
  const i = types.indexOf(contentType);
  if (i >= 0) types.splice(i, 1);
  else types.push(contentType);
  rulesDraft.value = next;
  flushAutoTagRules();
}

function addAutoTagRule() {
  const next = cloneRules(rulesDraft.value);
  next.push({ tag_name: "", keywords: [], content_types: [] });
  rulesDraft.value = next;
  flushAutoTagRules();
}

function removeAutoTagRule(index: number) {
  const next = cloneRules(rulesDraft.value);
  next.splice(index, 1);
  rulesDraft.value = next;
  flushAutoTagRules();
}

function restoreDefaultAutoTagRules() {
  rulesDraft.value = cloneRules(DEFAULT_AUTO_TAG_RULES);
  flushAutoTagRules();
}

function ruleAccentColor(tagName: string, index: number): string {
  const name = tagName.trim();
  const known = resolveKnownTagColors();
  if (name && known[name]) return known[name];
  const fromStore = clipboardStore.tags.find((t) => t.name === name)?.color;
  if (fromStore) return fromStore;
  const palette = resolveTagPalette();
  return palette[index % palette.length] ?? cssFallbackAccent();
}

function cssFallbackAccent(): string {
  return resolveTagPalette()[0] ?? "#0078d4";
}

onUnmounted(() => {
  if (rulesCommitTimer) flushAutoTagRules();
});
</script>

<style scoped>
.auto-tag-panel {
  margin-top: 4px;
  padding: 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.auto-tag-panel-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}

.auto-tag-panel-title {
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-primary);
}

.auto-tag-panel-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.auto-tag-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 28px 16px;
  text-align: center;
  color: var(--text-tertiary);
  border: 1px dashed var(--border-default);
  border-radius: var(--radius-md);
  background: var(--accent-softer);
}

.auto-tag-empty p {
  margin: 0;
  font-size: var(--text-md);
  line-height: 1.5;
  max-width: 260px;
}

.auto-tag-rules {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.auto-tag-rule {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}

.auto-tag-rule:hover {
  border-color: var(--border-default);
}

.auto-tag-rule:focus-within {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border-default));
  box-shadow: 0 0 0 3px var(--accent-softer);
}

.auto-tag-rule-top {
  display: flex;
  align-items: center;
  gap: 8px;
}

.auto-tag-rule-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.auto-tag-rule-index {
  flex: 1;
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-secondary);
}

.auto-tag-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  flex-shrink: 0;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.auto-tag-remove:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.auto-tag-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.auto-tag-field-label {
  font-size: var(--text-xs);
  font-weight: 500;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-tertiary);
}

.auto-tag-input-mono {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.auto-tag-keyword-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.auto-tag-chip {
  display: inline-flex;
  align-items: center;
  max-width: 100%;
  padding: 2px 7px;
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.auto-tag-chip-kw {
  background: var(--bg-active);
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.auto-tag-type-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.auto-tag-type-chip {
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

.auto-tag-type-chip:hover {
  border-color: var(--border-default);
  color: var(--text-primary);
}

.auto-tag-type-chip.active {
  --chip-accent: var(--accent);
  background: color-mix(in srgb, var(--chip-accent) 14%, transparent);
  border-color: color-mix(in srgb, var(--chip-accent) 45%, transparent);
  color: var(--chip-accent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--chip-accent) 20%, transparent);
}

.auto-tag-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-subtle);
}
</style>
