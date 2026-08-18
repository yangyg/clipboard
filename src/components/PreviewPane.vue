<template>
  <div class="preview-pane">
    <template v-if="record">
    <!-- Drawer close when parent hosts preview as overlay -->
    <button
      type="button"
      class="preview-drawer-close"
      :aria-label="$t('common.close')"
      :title="$t('common.close')"
      @click="clipboardStore.clearSelection()"
    >
      <AppIcon name="close" :size="14" />
    </button>

    <!-- Header -->
    <PreviewHeader
      :record="record"
      :type-label="typeLabel"
      :record-alias="recordAlias"
      :format-date-time="formatDateTime"
      @edit-alias="aliasDialogVisible = true"
    />

    <!-- Sensitive Warning -->
    <div v-if="record.is_sensitive" class="sensitive-warning" role="alert">
      <AppIcon name="warning" :size="14" />
      <span>{{ $t('preview.sensitiveContent') }}</span>
      <span class="auto-expire" v-if="expireText" :title="expireTitle || undefined">
        {{ expireText }}
      </span>
    </div>

    <!-- Content Preview -->
    <PreviewContent
      :key="record.id"
      :record="record"
      :show-html-preview="showHtmlPreview"
      :sanitized-html="sanitizedHtml"
      :clipboard-color="clipboardColor"
      :plain-content-html="plainContentHtml"
      :safe-link-href="safeLinkHref"
      :openable-link-url="openableLinkUrl"
      :link-title="linkTitle"
      :image-src="imageSrc"
      @open-image="openImageExternally"
      @open-link="openLinkExternally"
    />

    <!-- Tags -->
    <PreviewTags
      v-if="tagsEnabled"
      :record="record"
      :get-tag-bg="getTagBg"
      :get-tag-color="getTagColor"
      @remove-tag="removeTag"
      @open-assign="openTagAssign"
    />

    <!-- Tag Dialog (for assigning tags to this record) -->
    <TagDialog
      v-if="tagsEnabled"
      :visible="tagDialogVisible"
      :mode="tagDialogMode"
      :recordId="record.id"
      @close="tagDialogVisible = false"
      @switchToCreate="tagDialogMode = 'create'"
      @created="onTagCreated"
    />

    <AliasDialog
      :visible="aliasDialogVisible"
      :record-id="record.id"
      :initial-alias="record.alias ?? ''"
      @close="aliasDialogVisible = false"
    />

    <!-- Actions -->
    <PreviewActionBar
      v-if="record"
      :record="record"
      :pinned-display="pinnedDisplay"
      @paste="paste"
      @paste-plain="pastePlain"
      @favorite="favorite"
      @pin="pin"
      @delete="del"
      @restore="restore"
      @permanent-delete="permanentDel"
    />
    </template>

    <!-- Empty: host mounted without a selection (should not happen; list hides the pane). -->
    <div v-else class="preview-empty">
      <div class="preview-empty-icon"><AppIcon name="clipboard" :size="36" :stroke-width="1.5" /></div>
      <div class="preview-empty-text">{{ $t('preview.empty') }}</div>
      <div class="preview-empty-hint">{{ $t('preview.emptyHint') }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, toRef } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import TagDialog from "./TagDialog.vue";
import AliasDialog from "./AliasDialog.vue";
import PreviewActionBar from "./PreviewActionBar.vue";
import PreviewContent from "./PreviewContent.vue";
import PreviewHeader from "./PreviewHeader.vue";
import PreviewTags from "./PreviewTags.vue";
import AppIcon from "./icons/AppIcon.vue";
import { useFeature } from "../composables/useFeature";
import { recordMediaSrc } from "../utils/mediaUrl";
import { sanitizeClipboardHtml } from "../utils/sanitizeHtml";
import { escapeHtml, highlightSearchHtml } from "../utils/highlightSearch";
import { parseClipboardColor } from "../utils/clipboardColor";
import { useI18n } from "vue-i18n";
import { useExpireCountdown } from "../composables/useExpireCountdown";
import { usePreviewFormatting } from "../composables/usePreviewFormatting";
import { usePreviewActions } from "../composables/usePreviewActions";

const tagsEnabled = useFeature("tags");

const clipboardStore = useClipboardStore();
const { t } = useI18n();
const record = computed(() => clipboardStore.selectedRecord);
const imageSrc = computed(() => (record.value ? recordMediaSrc(record.value) : null));

/**
 * Show rich HTML only when it adds real formatting (Word etc.).
 * Share links (Douyin) usually wrap the same caption in <a>/<p>/<span> — use plain text.
 */
const showHtmlPreview = computed(() => {
  const html = record.value?.content_html;
  if (!html) return false;
  const stripped = html
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/gi, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!stripped) return false;
  // Require formatting beyond link/paragraph wrappers
  if (!/<(b|i|strong|em|u|ul|ol|li|h[1-6]|table|img|font)\b/i.test(html)
    && !/<span\b[^>]*\bstyle\s*=/i.test(html)) {
    return false;
  }
  const plain = (record.value?.content || "").replace(/\s+/g, " ").trim();
  // Same body as plain text → no benefit to a second HTML view
  if (plain && (stripped === plain || plain.includes(stripped) || stripped.includes(plain))) {
    return false;
  }
  return true;
});

const sanitizedHtml = computed(() => {
  const html = record.value?.content_html;
  if (!html || !showHtmlPreview.value) return "";
  return sanitizeClipboardHtml(html);
});

/** http(s) only — safe as WebView <a href>. */
const safeLinkHref = computed(() => {
  const raw = (record.value?.content ?? "").trim();
  if (!raw) return null;
  try {
    const u = new URL(raw);
    if (u.protocol === "http:" || u.protocol === "https:") return raw;
  } catch {
    /* ignore */
  }
  return null;
});

/** Download / OS-handler schemes (magnet, ed2k, thunder, ftp) — open via Rust. */
const OPENABLE_LINK_PREFIXES = [
  "https://",
  "http://",
  "ftp://",
  "magnet:",
  "ed2k://",
  "thunder://",
] as const;

const openableLinkUrl = computed(() => {
  const raw = (record.value?.content ?? "").trim();
  if (!raw) return null;
  const lower = raw.toLowerCase();
  for (const p of OPENABLE_LINK_PREFIXES) {
    if (lower.startsWith(p) && raw.length > p.length) return raw;
  }
  return null;
});

const linkTitle = computed(() => {
  if (safeLinkHref.value) return t("preview.webLink");
  if (openableLinkUrl.value) return t("preview.downloadLink");
  return t("preview.webLink");
});

/** Plain content with optional search-term highlighting (escaped). */
const plainContentHtml = computed(() => {
  const text = record.value?.content ?? "";
  const q = clipboardStore.searchQuery.trim();
  if (!q) return escapeHtml(text);
  return highlightSearchHtml(text, q);
});

/** Standalone CSS color in text → preview swatch (not a content_type). */
const clipboardColor = computed(() => {
  if (!record.value || record.value.content_type !== "text") return null;
  if (showHtmlPreview.value) return null;
  return parseClipboardColor(record.value.content);
});

const { recordAlias, typeLabel, tagsByName, getTagBg, getTagColor, formatDateTime } =
  usePreviewFormatting(record, toRef(clipboardStore, "tags"));
const { expireText, expireTitle } = useExpireCountdown(record);
const {
  pinnedDisplay,
  tagDialogVisible,
  tagDialogMode,
  openImageExternally,
  openLinkExternally,
  openTagAssign,
  removeTag,
  onTagCreated,
  paste,
  pastePlain,
  favorite,
  pin,
  del,
  restore,
  permanentDel,
} = usePreviewActions({ record, openableLinkUrl, tagsByName });

const aliasDialogVisible = ref(false);
</script>

<style scoped>
.preview-pane {
  position: relative;
  flex: 1;
  min-width: 0;
  width: 100%;
  height: 100%;
  background: var(--bg-surface);
  border-left: none;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.preview-drawer-close {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 2;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  cursor: pointer;
  font-family: inherit;
  box-shadow: var(--shadow-sm);
}

.preview-drawer-close:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.preview-pane:deep(.preview-header) {
  padding-right: 48px;
}

/* Sensitive Warning */
.sensitive-warning {
  background: var(--danger-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  padding: var(--space-2) var(--space-3);
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  row-gap: var(--space-1);
  font-size: var(--text-sm);
  color: var(--danger);
  flex-shrink: 0;
}

.auto-expire {
  margin-left: auto;
  font-size: var(--text-xs);
  opacity: 0.8;
}

/* Empty state: same pure-CSS-animation pattern as ListEmptyState — a JS
   <Transition> can stall unmounted while the WebView2 window is hidden. */
.preview-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-5);
  text-align: center;
  color: var(--text-tertiary);
  font-size: var(--text-md);
  animation: preview-empty-in var(--transition-smooth) ease;
}

@keyframes preview-empty-in {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

:global(body.anim-disabled) .preview-empty {
  animation: none;
}

@media (prefers-reduced-motion: reduce) {
  .preview-empty {
    animation: none;
  }
}

.preview-empty-icon {
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

.preview-empty-text {
  font-size: var(--text-base);
}

.preview-empty-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

@media (max-width: 560px) {
  .preview-pane {
    min-width: 0;
  }
}

</style>
