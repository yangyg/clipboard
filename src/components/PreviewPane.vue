<template>
  <div class="preview-pane" v-if="record">
    <!-- Header -->
    <div class="preview-header">
      <div class="preview-type-row">
        <div class="preview-type-icon" :class="record.content_type" :title="`内容类型：${typeLabel}`">
          <TypeIcon :type="record.content_type" :size="14" />
        </div>
        <div class="preview-heading">
          <div class="preview-name" :title="`内容类型：${typeLabel}`">{{ typeLabel }}</div>
          <div class="preview-meta-line">
            <span :title="`来源：${record.source_app || '系统剪贴板'}`">{{ record.source_app || '系统剪贴板' }}</span>
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="`创建时间：${formatDateTime(record.created_at)}`">{{ formatDateTime(record.created_at) }}</span>
            <template v-if="record.content_type === 'image' && record.width && record.height">
              <span class="meta-sep" aria-hidden="true">·</span>
              <span :title="`尺寸：${record.width}×${record.height}`">{{ record.width }}×{{ record.height }}</span>
            </template>
            <template v-else>
              <span class="meta-sep" aria-hidden="true">·</span>
              <span :title="`字符数：${record.content_len ?? record.content.length}`">{{ record.content_len ?? record.content.length }} 字符</span>
            </template>
            <template v-if="record.content_html">
              <span class="meta-sep" aria-hidden="true">·</span>
              <span title="格式：保留富文本">富文本</span>
            </template>
            <span class="meta-sep" aria-hidden="true">·</span>
            <span :title="`从本应用粘贴次数：${record.copy_count}`">粘贴 {{ record.copy_count }} 次</span>
          </div>
        </div>
        <button
          v-if="pinnedDisplay"
          type="button"
          class="preview-action-btn preview-pin-btn active"
          @click="pin"
          title="取消置顶"
          aria-label="取消置顶"
          :aria-pressed="true"
        ><AppIcon name="pin" :size="13" fill="currentColor" /></button>
        <button
          v-if="record.is_favorite"
          type="button"
          class="preview-action-btn active"
          @click="favorite"
          title="取消收藏"
          aria-label="取消收藏"
          :aria-pressed="true"
        ><AppIcon name="star" :size="13" fill="currentColor" /></button>
      </div>
    </div>

    <!-- Sensitive Warning -->
    <div v-if="record.is_sensitive" class="sensitive-warning">
      <AppIcon name="warning" :size="14" />
      <span>敏感内容</span>
      <span class="auto-expire" v-if="record.auto_expire_at">
        {{ formatExpireTime(record.auto_expire_at) }} 后自动删除
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
          <div class="link-title">网页链接</div>
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
            alt="剪贴板图片"
            class="image-thumb"
            title="点击用系统查看器打开"
            loading="lazy"
            decoding="async"
            @click.stop="openImageExternally"
          />
          <div v-else class="image-placeholder"><AppIcon name="image" :size="28" /> 暂无图片数据</div>
        </div>
      </template>
    </div>

    <!-- Tags -->
    <div class="preview-tags">
      <div class="tags-label">标签</div>
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
            class="tag-remove"
            @click.stop="removeTag(tag)"
            title="移除标签"
          ><AppIcon name="close" :size="10" /></button>
        </span>
        <button class="tag-add-btn" @click="openTagAssign"><AppIcon name="plus" :size="12" /> 添加标签</button>
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

    <!-- Actions -->
    <div class="preview-actions" v-if="record && !record.is_trashed">
      <button type="button" class="action-btn action-primary" @click="paste">
        <span class="action-icon"><AppIcon name="paste" :size="15" /></span>
        <span class="action-label">粘贴</span>
      </button>
      <button type="button" class="action-btn" @click="pastePlain">
        <span class="action-icon"><AppIcon name="type" :size="15" /></span>
        <span class="action-label">纯文本</span>
      </button>
      <button
        type="button"
        class="action-btn"
        :class="{ 'action-active': record.is_favorite }"
        @click="favorite"
      >
        <span class="action-icon"><AppIcon name="star" :size="15" :fill="record.is_favorite ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ record.is_favorite ? '已收藏' : '收藏' }}</span>
      </button>
      <button
        type="button"
        class="action-btn"
        :class="{ 'action-active': pinnedDisplay }"
        @click="pin"
      >
        <span class="action-icon"><AppIcon name="pin" :size="15" :fill="pinnedDisplay ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ pinnedDisplay ? '已置顶' : '置顶' }}</span>
      </button>
      <button type="button" class="action-btn action-icon-only danger" aria-label="删除" title="删除" @click="del">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
      </button>
    </div>
    <div class="preview-actions trash-actions" v-if="record && record.is_trashed">
      <button type="button" class="action-btn action-primary" @click="restore">
        <span class="action-icon"><AppIcon name="restore" :size="15" /></span>
        <span class="action-label">恢复</span>
      </button>
      <button type="button" class="action-btn action-icon-only danger" aria-label="永久删除" title="永久删除" @click="permanentDel">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import TagDialog from "./TagDialog.vue";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useSettingsStore } from "../stores/settings";
import { invoke } from "@tauri-apps/api/core";
import { recordMediaSrc } from "../utils/mediaUrl";
import { sanitizeClipboardHtml } from "../utils/sanitizeHtml";
import { escapeHtml, highlightSearchHtml } from "../utils/highlightSearch";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const { toast } = useToast();
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
    const msg = typeof e === "string" ? e : "无法用系统查看器打开";
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

const tagDialogVisible = ref(false);
const tagDialogMode = ref<"assign" | "create">("assign");

const TYPE_LABELS: Record<string, string> = {
  text: "纯文本",
  code: "代码片段",
  link: "链接",
  image: "图片",
  file: "文件路径",
  sensitive: "敏感内容",
};

const typeLabel = computed(() => {
  if (!record.value) return "";
  if (record.value.is_sensitive) return "敏感内容";
  return TYPE_LABELS[record.value.content_type] ?? "文本片段";
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
  if (ms <= 0) return "已过期";
  const totalSec = Math.ceil(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  if (m > 0) return `${m} 分 ${String(s).padStart(2, "0")} 秒`;
  return `${s} 秒`;
}

function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString("zh-CN", {
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
    toast(mode === "plain" ? "已粘贴为纯文本" : "已粘贴", "success");
  } catch {
    toast("粘贴失败", "error");
  }
}

async function pastePlain() {
  if (!record.value) return;
  try {
    await clipboardStore.pasteRecord(record.value.id, "plain");
    toast("已粘贴为纯文本", "success");
  } catch {
    toast("粘贴失败", "error");
  }
}

async function favorite() {
  if (!record.value) return;
  const next = await clipboardStore.toggleFavorite(record.value.id);
  if (next == null) toast("操作失败", "error");
}

async function pin() {
  if (!record.value) return;
  const id = record.value.id;
  pinOverride.value = !pinnedDisplay.value;
  await new Promise((r) => setTimeout(r, 150));
  if (clipboardStore.selectedId !== id) {
    pinOverride.value = null;
    return;
  }
  const next = await clipboardStore.togglePin(id);
  pinOverride.value = null;
  if (next == null) toast("操作失败", "error");
}

async function del() {
  if (!record.value) return;
  await clipboardStore.deleteRecord(record.value.id);
  toast("已移到回收站", "success");
}

async function restore() {
  if (!record.value) return;
  await clipboardStore.restoreRecord(record.value.id);
}

async function permanentDel() {
  if (!record.value) return;
  const ok = await confirm({
    title: "永久删除",
    message: "确定要永久删除这条记录吗？此操作不可恢复。",
    confirmText: "永久删除",
    danger: true,
  });
  if (ok) {
    await clipboardStore.permanentlyDeleteRecord(record.value.id);
    toast("已永久删除", "success");
  }
}
</script>

<style scoped>
.preview-pane {
  flex: 1.15;
  min-width: 280px;
  width: auto;
  background: var(--bg-card, var(--bg-surface));
  border-left: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Header */
.preview-header {
  padding: 16px 20px 12px;
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
  font-size: 0.875rem;
  font-weight: 600;
  flex-shrink: 0;
}

.preview-type-icon.text {
  background: color-mix(in srgb, var(--type-text) 15%, transparent);
  color: var(--type-text);
}

.preview-type-icon.code {
  background: color-mix(in srgb, var(--type-code) 15%, transparent);
  color: var(--type-code);
}

.preview-type-icon.link {
  background: color-mix(in srgb, var(--type-link) 15%, transparent);
  color: var(--type-link);
}

.preview-type-icon.image {
  background: color-mix(in srgb, var(--type-image) 15%, transparent);
  color: var(--type-image);
}

.preview-type-icon.file {
  background: color-mix(in srgb, var(--type-file) 15%, transparent);
  color: var(--type-file);
}

.preview-heading {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.preview-name {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--text-primary);
}

.preview-meta-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0 2px;
  font-size: 0.688rem;
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
  margin: 0 4px;
  opacity: 0.7;
}

.preview-action-btn {
  font-size: 1rem;
  color: var(--text-muted, var(--text-tertiary));
  background: none;
  border: none;
  cursor: pointer;
  padding: 0 3px;
  line-height: 1;
  transition: color var(--transition-fast);
}

.preview-action-btn:hover {
  color: var(--accent);
}

.preview-action-btn.active {
  color: var(--warning);
}

.preview-pin-btn.active {
  color: var(--accent);
}

.preview-more {
  font-size: 1rem;
  color: var(--text-muted, var(--text-tertiary));
  background: none;
  border: none;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

/* Sensitive Warning */
.sensitive-warning {
  background: var(--danger-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  padding: 6px 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.688rem;
  color: var(--danger);
  flex-shrink: 0;
}

.auto-expire {
  margin-left: auto;
  font-size: 0.625rem;
  opacity: 0.8;
}

/* Content */
.preview-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: 16px 20px;
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
  background: var(--bg-surface);
  border: 1px solid var(--border-light, var(--border-subtle));
  border-radius: var(--radius-md, 10px);
  padding: 14px 16px;
  font-size: 0.813rem;
  line-height: 1.65;
  color: var(--text-primary);
  max-width: 100%;
  min-width: 0;
  overflow-x: hidden;
  word-break: break-word;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.code-box {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  line-height: 1.6;
  background: var(--code-bg);
  color: var(--text-primary);
  border: none;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.link-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  padding: 16px;
  border: 1px solid var(--border-light, var(--border-subtle));
  border-radius: var(--radius-md, 10px);
  background: var(--bg-surface);
}

.link-icon {
  font-size: 1.375rem;
  opacity: 0.8;
}

.link-title {
  font-size: 0.813rem;
  font-weight: 700;
  color: var(--text-primary);
}

.link-url {
  color: var(--accent);
  font-size: 0.75rem;
  word-break: break-all;
  text-decoration: none;
}

.link-url:hover {
  text-decoration: underline;
}

.file-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-light, var(--border-subtle));
  background: var(--bg-surface);
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
  font-size: 0.75rem;
  color: var(--text-secondary);
  word-break: break-all;
}

.image-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.image-placeholder {
  padding: 20px;
  background: var(--bg-surface);
  border-radius: var(--radius-md, 10px);
  font-size: 2rem;
  opacity: 0.5;
}

.image-card img,
.image-thumb {
  max-width: 100%;
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-default);
  cursor: zoom-in;
}

/* Tags */
.preview-tags {
  padding: 0 20px 12px;
}

.tags-label {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.tags-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.tag-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: 20px;
  font-size: 0.719rem;
  font-weight: 500;
}

.tag-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.tag-remove {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: transparent;
  color: inherit;
  opacity: 0.6;
  font-size: 0.563rem;
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
  gap: 4px;
  padding: 4px 10px;
  border-radius: 20px;
  font-size: 0.719rem;
  color: var(--text-muted, var(--text-tertiary));
  cursor: pointer;
  border: 1px dashed var(--border-default, var(--border-subtle));
  background: transparent;
  transition: all var(--transition-fast);
}

.tag-add-btn:hover {
  color: var(--accent);
  border-color: var(--accent);
}

/* Action Buttons */
.preview-actions {
  padding: 12px 20px 20px;
  border-top: 1px solid var(--border-light, var(--border-subtle));
  display: grid;
  grid-template-columns: 1.5fr repeat(3, 1fr) auto;
  gap: 8px;
}

.action-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 10px 4px;
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
  color: var(--accent);
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

.action-btn.action-active:hover .action-label,
.action-btn.action-active:hover .action-icon {
  color: var(--warning);
}

.action-btn.action-icon-only {
  padding: 10px;
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
  font-size: 1.125rem;
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
    grid-template-columns: 1.4fr 1fr 1fr auto;
  }

  .preview-actions:not(.trash-actions) .action-btn:nth-child(4) {
    display: none;
  }
}
</style>
