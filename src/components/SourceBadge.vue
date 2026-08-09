<template>
  <span class="source-badge" :title="resolvedTitle">
    <img
      v-if="iconSrc"
      class="source-avatar source-avatar--img"
      :src="iconSrc"
      alt=""
      aria-hidden="true"
    />
    <span
      v-else
      class="source-avatar"
      :style="{ background: badge.color }"
      aria-hidden="true"
    >{{ badge.initial }}</span>
    <span
      v-if="labelHtml != null && labelHtml !== ''"
      class="source-label"
      v-html="labelHtml"
    />
    <span v-else class="source-label">{{ badge.label }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";
import { buildSourceOverrides, resolveSourceBadge } from "../utils/sourceBadge";

const { t } = useI18n();
const settingsStore = useSettingsStore();

const props = defineProps<{
  sourceApp: string;
  /** FileDescription-based friendly name from Rust capture (optional). */
  sourceName?: string;
  /** Full tooltip; defaults to `Source: {display} ({raw})`. */
  title?: string;
  /** Pre-highlighted / escaped HTML for the label (search). */
  labelHtml?: string;
  /** Reserved for future real app icons. */
  iconSrc?: string;
}>();

const overrides = computed(() =>
  buildSourceOverrides(settingsStore.settings.source_name_overrides),
);

const badge = computed(() =>
  resolveSourceBadge(props.sourceApp ?? "", props.sourceName, t, overrides.value)
);

const resolvedTitle = computed(() => {
  if (props.title != null && props.title !== "") return props.title;
  const raw = (props.sourceApp || "").trim();
  if (!raw) return t('record.systemClipboard');
  const label = badge.value.label;
  return label === raw
    ? t('record.sourceTooltip', { app: raw })
    : t('record.sourceTooltip', { app: `${label} (${raw})` });
});
</script>

<style scoped>
.source-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  max-width: 100%;
  vertical-align: middle;
}

.source-avatar {
  box-sizing: border-box;
  flex-shrink: 0;
  width: 14px;
  height: 14px;
  border-radius: var(--radius-xs);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-xs);
  font-weight: 600;
  line-height: 1;
  color: #fff;
  user-select: none;
}

.source-avatar--img {
  object-fit: cover;
  padding: 0;
  background: transparent;
}

.source-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
