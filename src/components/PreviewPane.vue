<template>
  <div class="preview-pane" v-if="record">
    <!-- Drawer close when parent hosts preview as overlay -->
    <button
      v-if="drawer"
      type="button"
      class="preview-drawer-close"
      :aria-label="$t('common.close')"
      :title="$t('common.close')"
      @click="clipboardStore.clearSelection()"
    >
      <AppIcon name="close" :size="14" />
    </button>

    <!-- Header -->
    <div class="preview-header">
      <div class="preview-type-row">
        <div class="preview-type-icon type-chip" :class="record.content_type" :title="$t('preview.contentType', { type: typeLabel })">
          <TypeIcon :type="record.content_type" :size="14" />
        </div>
        <div class="preview-heading">
          <div class="preview-name" :title="$t('preview.contentType', { type: typeLabel })">{{ typeLabel }}</div>
          <button
            v-if="!record.is_trashed"
            type="button"
            class="preview-alias-btn"
            :class="{ 'has-alias': !!recordAlias }"
            :title="recordAlias ? $t('preview.editAlias') : $t('preview.setAlias')"
            @click="aliasDialogVisible = true"
          >
            <AppIcon name="edit" :size="11" />
            <span>{{ recordAlias || $t('preview.setAlias') }}</span>
          </button>
          <div class="preview-meta-line">
            <SourceBadge :source-app="record.source_app" />
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="$t('preview.createdAt', { time: formatDateTime(record.created_at) })">{{ formatDateTime(record.created_at) }}</span>
            <template v-if="record.content_type === 'image' && record.width && record.height">
              <span class="meta-sep" aria-hidden="true">·</span>
              <span :title="$t('preview.dimensions', { w: record.width, h: record.height })">{{ record.width }}×{{ record.height }}</span>
            </template>
            <template v-else>
              <span class="meta-sep" aria-hidden="true">·</span>
              <span :title="$t('preview.charCount', { count: record.content_len ?? record.content.length })">{{ record.content_len ?? record.content.length }} {{ $t('common.chars') }}</span>
            </template>
            <template v-if="record.content_html">
              <span class="meta-sep" aria-hidden="true">·</span>
              <span :title="$t('preview.richTextTitle')">{{ $t('preview.richText') }}</span>
            </template>
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="$t('preview.pasteCountTitle', { count: record.copy_count })">{{ $t('preview.pasteCount', { count: record.copy_count }) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Sensitive Warning -->
    <div v-if="record.is_sensitive" class="sensitive-warning">
      <AppIcon name="warning" :size="14" />
      <span>{{ $t('preview.sensitiveContent') }}</span>
      <span class="auto-expire" v-if="record.auto_expire_at">
        {{ $t('preview.autoExpire', { time: formatExpireTime(record.auto_expire_at) }) }}
      </span>
    </div>

    <!-- Content Preview -->
    <div class="preview-content">
      <!-- Text: HTML preview XOR plain — never both (Douyin shares duplicate otherwise). -->
      <template v-if="record.content_type === 'text'">
        <div
          v-if="showHtmlPreview"
          class="content-box html-preview"
          v-html="sanitizedHtml"
        />
        <div v-else-if="clipboardColor" class="color-preview-card">
          <div
            class="color-swatch-lg"
            :style="{ background: clipboardColor }"
            :title="clipboardColor"
            aria-hidden="true"
          />
          <div class="content-box color-value" v-html="plainContentHtml"></div>
        </div>
        <div v-else class="content-box" v-html="plainContentHtml"></div>
      </template>

      <!-- Code -->
      <template v-else-if="record.content_type === 'code'">
        <pre class="content-box code-box" v-html="plainContentHtml"></pre>
      </template>

      <!-- Link -->
      <template v-else-if="record.content_type === 'link'">
        <div class="link-card">
          <div class="link-icon"><AppIcon name="link" :size="22" /></div>
          <div class="link-title">{{ $t('preview.webLink') }}</div>
          <a
            v-if="safeLinkHref"
            class="link-url"
            :href="safeLinkHref"
            target="_blank"
            rel="noopener noreferrer"
            v-html="plainContentHtml"
          ></a>
          <div v-else class="link-url" v-html="plainContentHtml"></div>
        </div>
      </template>

      <!-- File -->
      <template v-else-if="record.content_type === 'file'">
        <div class="file-card">
          <div class="file-icon"><AppIcon name="file" :size="22" /></div>
          <div class="file-path" v-html="plainContentHtml"></div>
        </div>
      </template>

      <!-- Image -->
      <template v-else-if="record.content_type === 'image'">
        <div class="image-card">
          <img
            v-if="imageSrc"
            :src="imageSrc"
            :alt="$t('preview.clipboardImage')"
            class="image-thumb"
            :title="$t('preview.clickToOpen')"
            tabindex="0"
            role="button"
            loading="lazy"
            decoding="async"
            @click.stop="openImageExternally"
            @keydown.enter.prevent="openImageExternally"
            @keydown.space.prevent="openImageExternally"
          />
          <div v-else class="image-placeholder"><AppIcon name="image" :size="28" /> {{ $t('preview.noImageData') }}</div>
        </div>
      </template>
    </div>

    <!-- Tags -->
    <div class="preview-tags">
      <div class="tags-label">{{ $t('preview.tags') }}</div>
      <div class="tags-list">
        <span
          v-for="tag in record.tags"
          :key="tag"
          class="tag-chip"
          :style="{ background: getTagBg(tag), color: getTagColor(tag) }"
        >
          <span class="tag-dot" :style="{ background: getTagColor(tag) }"></span>
          {{ tag }}
          <button
            type="button"
            class="tag-remove"
            @click.stop="removeTag(tag)"
            :aria-label="$t('preview.removeTag')"
            :title="$t('preview.removeTag')"
          ><AppIcon name="close" :size="10" /></button>
        </span>
        <button class="tag-add-btn" @click="openTagAssign"><AppIcon name="plus" :size="12" /> {{ $t('preview.addTag') }}</button>
      </div>
    </div>

    <!-- Tag Dialog (for assigning tags to this record) -->
    <TagDialog
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
    <div class="preview-actions" v-if="record && !record.is_trashed">
      <button type="button" class="action-btn action-primary" @click="paste">
        <span class="action-icon"><AppIcon name="paste" :size="15" /></span>
        <span class="action-label">{{ $t('preview.paste') }}</span>
      </button>
      <button type="button" class="action-btn" @click="pastePlain">
        <span class="action-icon"><AppIcon name="type" :size="15" /></span>
        <span class="action-label">{{ $t('preview.plainText') }}</span>
      </button>
      <button
        type="button"
        class="action-btn"
        :class="{ 'action-active': record.is_favorite }"
        @click="favorite"
      >
        <span class="action-icon"><AppIcon name="star" :size="15" :fill="record.is_favorite ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ record.is_favorite ? $t('preview.favorited') : $t('preview.favorite') }}</span>
      </button>
      <button
        type="button"
        class="action-btn"
        :class="{ 'action-pinned': pinnedDisplay }"
        @click="pin"
      >
        <span class="action-icon"><AppIcon name="pin" :size="15" :fill="pinnedDisplay ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ pinnedDisplay ? $t('preview.pinned') : $t('preview.pin') }}</span>
      </button>
      <button type="button" class="action-btn action-icon-only danger" :aria-label="$t('preview.deleteBtn')" :title="$t('preview.deleteBtn')" @click="del">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
      </button>
    </div>
    <div class="preview-actions trash-actions" v-if="record && record.is_trashed">
      <button type="button" class="action-btn action-primary" @click="restore">
        <span class="action-icon"><AppIcon name="restore" :size="15" /></span>
        <span class="action-label">{{ $t('preview.restoreBtn') }}</span>
      </button>
      <button type="button" class="action-btn action-icon-only danger" :aria-label="$t('preview.permanentDelete')" :title="$t('preview.permanentDelete')" @click="permanentDel">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import TagDialog from "./TagDialog.vue";
import AliasDialog from "./AliasDialog.vue";
import SourceBadge from "./SourceBadge.vue";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useSettingsStore } from "../stores/settings";
import { invoke } from "@tauri-apps/api/core";
import { recordMediaSrc } from "../utils/mediaUrl";
import { sanitizeClipboardHtml } from "../utils/sanitizeHtml";
import { escapeHtml, highlightSearchHtml } from "../utils/highlightSearch";
import { parseClipboardColor } from "../utils/clipboardColor";
import { useI18n } from "vue-i18n";

withDefaults(
  defineProps<{
    /** When true, show a close control (parent is hosting as overlay drawer). */
    drawer?: boolean;
  }>(),
  { drawer: false },
);

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();
const record = computed(() => clipboardStore.selectedRecord);
const imageSrc = computed(() => (record.value ? recordMediaSrc(record.value) : null));

/** Optimistic pin label/icon before list reorders. */
const pinOverride = ref<boolean | null>(null);
watch(
  () => record.value?.id,
  () => {
    pinOverride.value = null;
  },
);

const pinnedDisplay = computed(() => {
  if (pinOverride.value !== null) return pinOverride.value;
  return !!record.value?.is_pinned;
});

async function openImageExternally() {
  const id = record.value?.id;
  if (id == null) return;
  try {
    await invoke("open_record_media", { id });
  } catch (e) {
    console.error("Open image failed:", e);
    const msg = typeof e === "string" ? e : t('preview.openImageFailed');
    toast(msg, "error");
  }
}

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

/** Only http(s) — blocks javascript:/data: from malicious imports. */
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

const tagDialogVisible = ref(false);
const tagDialogMode = ref<"assign" | "create">("assign");
const aliasDialogVisible = ref(false);

const recordAlias = computed(() => (record.value?.alias ?? "").trim());

const TYPE_LABEL_KEYS: Record<string, string> = {
  text: "preview.typeText",
  code: "preview.typeCode",
  link: "preview.typeLink",
  image: "preview.typeImage",
  file: "preview.typeFile",
  sensitive: "preview.typeSensitive",
};

const typeLabel = computed(() => {
  if (!record.value) return "";
  if (record.value.is_sensitive) return t('preview.typeSensitive');
  return t(TYPE_LABEL_KEYS[record.value.content_type] ?? 'preview.typeDefault');
});

const tagsByName = computed(() => {
  const map = new Map<string, (typeof clipboardStore.tags)[number]>();
  for (const t of clipboardStore.tags) map.set(t.name, t);
  return map;
});

function getTagBg(tagName: string): string {
  const tag = tagsByName.value.get(tagName);
  if (!tag) return "var(--bg-surface)";
  // Normalize hex color for CSS color-mix
  const hex = normalizeHex(tag.color);
  return `color-mix(in srgb, ${hex} 10%, transparent)`;
}

function normalizeHex(color: string): string {
  if (color.startsWith("#")) {
    if (color.length === 4) {
      // #abc -> #aabbcc
      return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`;
    }
    return color; // #rrggbb or #rrggbbaa
  }
  return color; // rgb()/rgba() passed through as-is
}

function getTagColor(tagName: string): string {
  return tagsByName.value.get(tagName)?.color ?? "var(--text-secondary)";
}

function openTagAssign() {
  tagDialogMode.value = "assign";
  tagDialogVisible.value = true;
}

async function removeTag(tagName: string) {
  if (!record.value) return;
  const tag = tagsByName.value.get(tagName);
  if (tag) {
    await clipboardStore.removeTagFromRecord(record.value.id, tag.id, tagName);
  }
}

function onTagCreated() {
  tagDialogMode.value = "assign";
}

const expireNow = ref(Date.now());
let expireTimer: ReturnType<typeof setInterval> | null = null;

function clearExpireTimer() {
  if (expireTimer) {
    clearInterval(expireTimer);
    expireTimer = null;
  }
}

watch(
  () => record.value?.auto_expire_at ?? null,
  (iso) => {
    clearExpireTimer();
    if (!iso) return;
    expireNow.value = Date.now();
    expireTimer = setInterval(() => {
      expireNow.value = Date.now();
    }, 1000);
  },
  { immediate: true }
);

onUnmounted(() => {
  clearExpireTimer();
});

/** Live countdown — always include seconds so the UI visibly ticks. */
function formatExpireTime(iso: string): string {
  const ms = new Date(iso).getTime() - expireNow.value;
  if (ms <= 0) return t('preview.expired');
  const totalSec = Math.ceil(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  if (m > 0) return `${m}:${String(s).padStart(2, "0")}`;
  return `${s}s`;
}

function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

async function paste() {
  if (!record.value) return;
  const mode = settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original";
  try {
    await clipboardStore.pasteRecord(record.value.id, mode);
    toast(mode === "plain" ? t('record.pastedPlain') : t('record.pasted'), "success");
  } catch {
    toast(t('record.pasteFailed'), "error");
  }
}

async function pastePlain() {
  if (!record.value) return;
  try {
    await clipboardStore.pasteRecord(record.value.id, "plain");
    toast(t('record.pastedPlain'), "success");
  } catch {
    toast(t('record.pasteFailed'), "error");
  }
}

async function favorite() {
  if (!record.value) return;
  const next = await clipboardStore.toggleFavorite(record.value.id);
  if (next == null) toast(t('common.operationFailed'), "error");
}

async function pin() {
  if (!record.value) return;
  const id = record.value.id;
  pinOverride.value = !pinnedDisplay.value;
  if (
    settingsStore.settings.enable_animation &&
    !window.matchMedia("(prefers-reduced-motion: reduce)").matches
  ) {
    await new Promise((r) => setTimeout(r, 150));
  }
  if (clipboardStore.selectedId !== id) {
    pinOverride.value = null;
    return;
  }
  const next = await clipboardStore.togglePin(id);
  pinOverride.value = null;
  if (next == null) toast(t('common.operationFailed'), "error");
}

async function del() {
  if (!record.value) return;
  try {
    await clipboardStore.deleteRecord(record.value.id);
    toast(t('record.deleted'), "success");
  } catch {
    toast(t('common.operationFailed'), "error");
  }
}

async function restore() {
  if (!record.value) return;
  try {
    await clipboardStore.restoreRecord(record.value.id);
  } catch {
    toast(t('common.operationFailed'), "error");
  }
}

async function permanentDel() {
  if (!record.value) return;
  const ok = await confirm({
    title: t('record.permanentDelete'),
    message: t('record.permanentDeleteMsg'),
    confirmText: t('record.permanentDelete'),
    danger: true,
  });
  if (ok) {
    try {
      await clipboardStore.permanentlyDeleteRecord(record.value.id);
      toast(t('record.deletedPermanently'), "success");
    } catch {
      toast(t('common.operationFailed'), "error");
    }
  }
}
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
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* Header */
.preview-header {
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-light, var(--border-subtle));
}

.preview-type-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.preview-type-icon {
  width: 40px;
  height: 40px;
  border-radius: var(--radius-md, 10px);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-lg);
  font-weight: 600;
  flex-shrink: 0;
}

/* Type icon coloring is provided by the shared .type-chip utility in
   main.css (single source of truth for content-type colors). */

.preview-heading {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.preview-name {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--text-primary);
}

.preview-alias-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-top: 2px;
  max-width: 100%;
  padding: 0;
  border: none;
  background: none;
  font-family: inherit;
  font-size: var(--text-md);
  font-weight: 500;
  color: var(--text-tertiary);
  cursor: pointer;
  text-align: left;
}

.preview-alias-btn span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-alias-btn:hover {
  color: var(--accent-text);
}

.preview-alias-btn.has-alias {
  color: var(--text-secondary);
}

.preview-meta-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0 2px;
  font-size: var(--text-sm);
  color: var(--text-muted, var(--text-tertiary));
  line-height: 1.35;
  overflow: hidden;
}

.preview-meta-line > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.meta-sep {
  flex-shrink: 0;
  margin: 0 var(--space-1);
  opacity: 0.7;
}

/* Sensitive Warning */
.sensitive-warning {
  background: var(--danger-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  padding: var(--space-2) var(--space-3);
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  color: var(--danger);
  flex-shrink: 0;
}

.auto-expire {
  margin-left: auto;
  font-size: var(--text-xs);
  opacity: 0.8;
}

/* Content */
.preview-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: var(--space-4) var(--space-5);
  overflow-x: hidden;
  overflow-y: auto;
}

/* Rich HTML often ships inline nowrap/fixed widths — force wrap inside preview. */
.html-preview {
  display: block;
  max-width: 100%;
  min-width: 0;
  overflow-x: hidden;
  white-space: normal !important;
}

.html-preview :deep(*) {
  max-width: 100% !important;
  min-width: 0 !important;
  box-sizing: border-box;
  white-space: normal !important;
  word-break: break-word !important;
  overflow-wrap: anywhere !important;
}

.html-preview :deep(img) {
  max-width: 100% !important;
  height: auto !important;
}

.html-preview :deep(pre),
.html-preview :deep(code) {
  white-space: pre-wrap !important;
}

.html-preview :deep(table) {
  width: 100% !important;
  table-layout: fixed;
  border-collapse: collapse;
}

.content-box {
  background: transparent;
  border: none;
  border-radius: 0;
  padding: var(--space-1) 0;
  font-size: var(--text-base);
  line-height: 1.65;
  color: var(--text-primary);
  max-width: 100%;
  min-width: 0;
  overflow-x: hidden;
  word-break: break-word;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.color-preview-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.color-swatch-lg {
  width: 100%;
  height: 96px;
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-subtle);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 12%, transparent);
  flex-shrink: 0;
}

.color-value {
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  letter-spacing: 0.02em;
  padding: 0;
}

.code-box {
  font-family: var(--font-mono);
  font-size: var(--text-md);
  line-height: 1.6;
  background: var(--code-bg);
  color: var(--text-primary);
  border: none;
  border-radius: var(--radius-md, 10px);
  padding: var(--space-3) var(--space-4);
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.link-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border: none;
  border-radius: var(--radius-md, 10px);
  background: var(--bg-elevated);
}

.link-icon {
  font-size: 1.375rem;
  opacity: 0.8;
}

.link-title {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--text-primary);
}

.link-url {
  color: var(--accent-text);
  font-size: var(--text-md);
  word-break: break-all;
  text-decoration: none;
}

.link-url:hover {
  text-decoration: underline;
}

.file-card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md, 10px);
  border: none;
  background: var(--bg-elevated);
}

.file-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--type-file) 15%, transparent);
  color: var(--type-file);
}

.file-path {
  font-family: var(--font-mono);
  font-size: var(--text-md);
  color: var(--text-secondary);
  word-break: break-all;
}

.image-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
}

.image-placeholder {
  padding: var(--space-5);
  background: var(--bg-elevated);
  border-radius: var(--radius-md, 10px);
  font-size: var(--text-3xl);
  opacity: 0.5;
}

.image-card img,
.image-thumb {
  max-width: 100%;
  border-radius: var(--radius-md, 10px);
  border: none;
  box-shadow: 0 0 0 1px var(--border-subtle);
  cursor: zoom-in;
}

/* Tags */
.preview-tags {
  padding: var(--space-2) var(--space-5) var(--space-4);
}

.tags-label {
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: var(--space-2);
}

.tags-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.tag-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-xl);
  font-size: var(--text-md);
  font-weight: 500;
}

.tag-dot {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-pill);
}

.tag-remove {
  width: 14px;
  height: 14px;
  border-radius: var(--radius-pill);
  background: transparent;
  color: inherit;
  opacity: 0.6;
  font-size: var(--text-xs);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: opacity var(--transition-fast);
  padding: 0;
  margin-left: 2px;
  border: none;
}

.tag-remove:hover {
  opacity: 1;
}

.tag-add-btn {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-xl);
  font-size: var(--text-md);
  color: var(--text-muted, var(--text-tertiary));
  cursor: pointer;
  border: 1px dashed var(--border-default, var(--border-subtle));
  background: transparent;
  transition: all var(--transition-fast);
}

.tag-add-btn:hover {
  color: var(--accent-text);
  border-color: var(--accent);
}

/* Action Buttons */
.preview-actions {
  padding: var(--space-2) var(--space-5) var(--space-5);
  display: grid;
  grid-template-columns: 1.5fr repeat(3, 1fr) auto;
  gap: var(--space-2);
}

.action-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-1);
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-default, var(--border-subtle));
  background: var(--bg-card, var(--bg-surface));
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.action-btn:hover {
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 25%, transparent);
}

.action-btn:active {
  transform: scale(0.96);
}

.action-btn:hover .action-label,
.action-btn:hover .action-icon {
  color: var(--accent-text);
}

.action-btn.action-primary {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.action-btn.action-primary .action-label,
.action-btn.action-primary .action-icon {
  color: #fff;
}

.action-btn.action-primary:hover {
  background: var(--accent-hover);
  border-color: var(--accent-hover);
}

.action-btn.action-primary:hover .action-label,
.action-btn.action-primary:hover .action-icon {
  color: #fff;
}

.action-btn.action-active {
  background: var(--warning-soft);
  border-color: color-mix(in srgb, var(--warning) 20%, transparent);
}

.action-btn.action-active .action-label,
.action-btn.action-active .action-icon {
  color: var(--warning);
}

.action-btn.action-active:hover {
  background: color-mix(in srgb, var(--warning) 28%, transparent);
  border-color: color-mix(in srgb, var(--warning) 45%, transparent);
}

.action-btn.action-active:hover .action-label,
.action-btn.action-active:hover .action-icon {
  color: var(--warning);
}

.action-btn.action-pinned {
  background: var(--pin-soft);
  border-color: color-mix(in srgb, var(--pin) 20%, transparent);
}

.action-btn.action-pinned .action-label,
.action-btn.action-pinned .action-icon {
  color: var(--pin);
}

.action-btn.action-pinned:hover {
  background: color-mix(in srgb, var(--pin) 28%, transparent);
  border-color: color-mix(in srgb, var(--pin) 45%, transparent);
}

.action-btn.action-pinned:hover .action-label,
.action-btn.action-pinned:hover .action-icon {
  color: var(--pin);
}

.action-btn.action-icon-only {
  padding: var(--space-3);
  min-width: 42px;
}

.action-btn.danger {
  color: var(--danger);
}

.action-btn.danger .action-icon {
  color: var(--danger);
}

.action-btn.danger:hover {
  background: var(--danger-soft);
  border-color: color-mix(in srgb, var(--danger) 20%, transparent);
}

.action-btn.danger:hover .action-label,
.action-btn.danger:hover .action-icon {
  color: var(--danger);
}

.trash-actions {
  grid-template-columns: 1.5fr auto;
}

.action-icon {
  font-size: var(--text-2xl);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  transition: color var(--transition-fast);
}

.action-label {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-secondary);
  transition: color var(--transition-fast);
}

@media (max-width: 720px) {
  .preview-actions:not(.trash-actions) {
    grid-template-columns: 1.4fr 1fr 1fr 1fr auto;
  }
}

@media (max-width: 560px) {
  .preview-pane {
    min-width: 0;
  }

  .preview-actions:not(.trash-actions) {
    grid-template-columns: 1.4fr 1fr 1fr auto;
  }

  /* Keep pin reachable via hotkey (Ctrl+T) / context menu when space is tight */
  .preview-actions:not(.trash-actions) .action-btn:nth-child(4) .action-label {
    display: none;
  }
}
</style>
