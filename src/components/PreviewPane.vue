<template>
  <div class="preview-pane" v-if="record">
    <!-- Header -->
    <div class="preview-header">
      <div class="preview-type-row">
        <div class="preview-type-icon" :class="record.content_type">
          <TypeIcon :type="record.content_type" :size="14" />
        </div>
        <div class="preview-name">{{ typeLabel }}</div>
        <button
          v-if="record.is_pinned"
          class="preview-action-btn preview-pin-btn active"
          @click="pin"
          title="取消置顶"
        ><AppIcon name="pin" :size="13" fill="currentColor" /></button>
        <button
          v-if="record.is_favorite"
          class="preview-action-btn active"
          @click="clipboardStore.toggleFavorite(record.id)"
          title="取消收藏"
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
    <div class="preview-content" ref="contentRef">
      <!-- Text: HTML preview XOR plain — never both (Douyin shares duplicate otherwise). -->
      <template v-if="record.content_type === 'text'">
        <iframe
          v-if="showHtmlPreview"
          class="html-preview"
          sandbox=""
          :srcdoc="htmlPreviewDoc"
          title="格式预览"
        />
        <div v-else class="content-box">{{ record.content }}</div>
      </template>

      <!-- Code -->
      <template v-else-if="record.content_type === 'code'">
        <pre class="content-box code-box">{{ record.content }}</pre>
      </template>

      <!-- Link -->
      <template v-else-if="record.content_type === 'link'">
        <div class="link-card">
          <div class="link-icon"><AppIcon name="link" :size="22" /></div>
          <div class="link-title">网页链接</div>
          <a class="link-url" :href="record.content" target="_blank">{{ record.content }}</a>
        </div>
      </template>

      <!-- File -->
      <template v-else-if="record.content_type === 'file'">
        <div class="file-card">
          <div class="file-icon"><AppIcon name="file" :size="22" /></div>
          <div class="file-path">{{ record.content }}</div>
        </div>
      </template>

      <!-- Image -->
      <template v-else-if="record.content_type === 'image'">
        <div class="image-card">
          <img
            v-if="imageSrc"
            :src="imageSrc"
            alt="剪贴板图片"
          />
          <div v-else class="image-placeholder"><AppIcon name="image" :size="28" /> 暂无图片数据</div>
        </div>
      </template>

      <!-- Meta Grid -->
      <div class="preview-meta">
        <div class="meta-item">
          <div class="meta-label">类型</div>
          <div class="meta-value">{{ TYPE_LABELS[record.content_type] || '文本' }}</div>
        </div>
        <div class="meta-item" v-if="record.content_type === 'image' && record.width && record.height">
          <div class="meta-label">尺寸</div>
          <div class="meta-value">{{ record.width }}×{{ record.height }}</div>
        </div>
        <div class="meta-item" v-else>
          <div class="meta-label">字符数</div>
          <div class="meta-value">{{ record.content.length }} 字符</div>
        </div>
        <div class="meta-item" v-if="record.content_html">
          <div class="meta-label">格式</div>
          <div class="meta-value">保留富文本</div>
        </div>
        <div class="meta-item">
          <div class="meta-label">创建时间</div>
          <div class="meta-value">{{ formatDateTime(record.created_at) }}</div>
        </div>
        <div class="meta-item">
          <div class="meta-label">来源</div>
          <div class="meta-value">{{ record.source_app || '系统剪贴板' }}</div>
        </div>
      </div>
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
      @created="tagDialogMode = 'assign'"
    />

    <!-- Actions -->
    <div class="preview-actions" v-if="record && !record.is_trashed">
      <button class="action-btn" @click="paste">
        <span class="action-icon"><AppIcon name="paste" :size="15" /></span>
        <span class="action-label">复制</span>
      </button>
      <button class="action-btn" @click="pastePlain">
        <span class="action-icon"><AppIcon name="type" :size="15" /></span>
        <span class="action-label">纯文本</span>
      </button>
      <button
        class="action-btn"
        :class="{ 'action-active': record.is_favorite }"
        @click="clipboardStore.toggleFavorite(record.id)"
      >
        <span class="action-icon"><AppIcon name="star" :size="15" :fill="record.is_favorite ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ record.is_favorite ? '已收藏' : '收藏' }}</span>
      </button>
      <button
        class="action-btn"
        :class="{ 'action-active': record.is_pinned }"
        @click="pin"
      >
        <span class="action-icon"><AppIcon name="pin" :size="15" :fill="record.is_pinned ? 'currentColor' : 'none'" /></span>
        <span class="action-label">{{ record.is_pinned ? '已置顶' : '置顶' }}</span>
      </button>
      <button class="action-btn danger" @click="del">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
        <span class="action-label">删除</span>
      </button>
    </div>
    <div class="preview-actions trash-actions" v-if="record && record.is_trashed">
      <button class="action-btn" @click="restore">
        <span class="action-icon"><AppIcon name="restore" :size="15" /></span>
        <span class="action-label">恢复</span>
      </button>
      <button class="action-btn danger" @click="permanentDel">
        <span class="action-icon"><AppIcon name="trash" :size="15" /></span>
        <span class="action-label">永久删除</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import TagDialog from "./TagDialog.vue";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import { useConfirm } from "../composables/useConfirm";
import { useSettingsStore } from "../stores/settings";
import { recordMediaSrc } from "../utils/mediaUrl";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const record = computed(() => clipboardStore.selectedRecord);
const imageSrc = computed(() => (record.value ? recordMediaSrc(record.value) : null));
/** Sandboxed preview document; inherits theme text color via body style. */
const htmlPreviewDoc = computed(() => {
  const html = record.value?.content_html;
  if (!html) return "";
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
html,body{margin:0;padding:0;background:transparent;}
body{padding:4px;font:13px/1.5 system-ui,sans-serif;color:CanvasText;word-break:break-word;}
img{max-width:100%;height:auto;}
</style></head><body>${html}</body></html>`;
});
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

function getTagBg(tagName: string): string {
  const tag = clipboardStore.tags.find((t) => t.name === tagName);
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
  const tag = clipboardStore.tags.find((t) => t.name === tagName);
  return tag?.color ?? "var(--text-secondary)";
}

function openTagAssign() {
  tagDialogMode.value = "assign";
  tagDialogVisible.value = true;
}

async function removeTag(tagName: string) {
  if (!record.value) return;
  const tag = clipboardStore.tags.find((t) => t.name === tagName);
  if (tag) {
    await clipboardStore.removeTagFromRecord(record.value.id, tag.id, tagName);
  }
}

function formatExpireTime(iso: string): string {
  const ms = new Date(iso).getTime() - Date.now();
  if (ms <= 0) return "已过期";
  const sec = Math.floor(ms / 1000);
  if (sec < 60) return `${sec} 秒`;
  return `${Math.floor(sec / 60)} 分钟`;
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

function paste() {
  if (!record.value) return;
  const mode = settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original";
  clipboardStore.pasteRecord(record.value.id, mode);
}

function pastePlain() {
  if (record.value) clipboardStore.pasteRecord(record.value.id, "plain");
}

function pin() {
  if (record.value) clipboardStore.togglePin(record.value.id);
}

async function del() {
  if (!record.value) return;
  const ok = await confirm({
    title: "移到回收站",
    message: "确定要将这条记录移到回收站吗？",
    confirmText: "删除",
    danger: true,
  });
  if (ok) await clipboardStore.deleteRecord(record.value.id);
}

function restore() {
  if (record.value) {
    clipboardStore.restoreRecord(record.value.id);
  }
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
    clipboardStore.permanentlyDeleteRecord(record.value.id);
  }
}
</script>

<style scoped>
.preview-pane {
  flex: 1.5;
  min-width: 300px;
  width: auto;
  background: var(--bg-card, var(--bg-surface));
  border-left: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
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
  margin-bottom: 10px;
}

.preview-type-icon {
  width: 40px;
  height: 40px;
  border-radius: var(--radius-md, 10px);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 600;
  flex-shrink: 0;
}

.preview-type-icon.text {
  background: rgba(79, 110, 247, 0.1);
  color: var(--accent);
}

.preview-type-icon.code {
  background: rgba(124, 92, 252, 0.1);
  color: #7c5cfc;
}

.preview-type-icon.link {
  background: rgba(23, 192, 146, 0.1);
  color: #17a97b;
}

.preview-type-icon.image {
  background: rgba(232, 125, 62, 0.1);
  color: #e87d3e;
}

.preview-type-icon.file {
  background: rgba(232, 106, 51, 0.1);
  color: #e86a33;
}

.preview-name {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
  flex: 1;
}

.preview-action-btn {
  font-size: 16px;
  color: var(--text-muted, var(--text-tertiary));
  background: none;
  border: none;
  cursor: pointer;
  padding: 0 3px;
  line-height: 1;
  transition: color var(--transition-fast), transform var(--transition-fast);
}

.preview-action-btn:hover {
  transform: scale(1.15);
  color: var(--text-primary);
}

.preview-action-btn.active {
  color: var(--warning);
}

.preview-pin-btn.active {
  color: var(--accent);
}

.preview-more {
  font-size: 16px;
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
  border-bottom: 1px solid rgba(242, 85, 85, 0.2);
  padding: 6px 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--danger);
  flex-shrink: 0;
}

.auto-expire {
  margin-left: auto;
  font-size: 10px;
  opacity: 0.8;
}

/* Content */
.preview-content {
  flex: 1;
  padding: 16px 20px;
  overflow-y: auto;
}

.html-preview {
  display: block;
  width: 100%;
  min-height: 120px;
  max-height: 280px;
  border: 1px solid var(--border-light, var(--border-subtle));
  border-radius: var(--radius-md, 10px);
  background: var(--bg-surface);
  color-scheme: inherit;
}

.content-box {
  background: var(--bg-surface);
  border: 1px solid var(--border-light, var(--border-subtle));
  border-radius: var(--radius-md, 10px);
  padding: 14px 16px;
  font-size: 13px;
  line-height: 1.65;
  color: var(--text-primary);
  word-break: break-word;
  white-space: pre-wrap;
  max-height: 280px;
  overflow-y: auto;
}

.code-box {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  line-height: 1.6;
  background: var(--code-bg);
  color: var(--text-primary);
  border: none;
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
  font-size: 22px;
  opacity: 0.8;
}

.link-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-primary);
}

.link-url {
  color: var(--accent);
  font-size: 12px;
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
  background: rgba(79, 110, 247, 0.1);
}

.file-path {
  font-family: var(--font-mono);
  font-size: 12px;
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
  font-size: 32px;
  opacity: 0.5;
}

.image-card img {
  max-width: 100%;
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-default);
}

/* Meta Grid */
.preview-meta {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-top: 14px;
}

.meta-item {
  background: var(--bg-surface);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
}

.meta-label {
  font-size: 10.5px;
  color: var(--text-muted, var(--text-tertiary));
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.meta-value {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: 2px;
  word-break: break-word;
}

/* Tags */
.preview-tags {
  padding: 0 20px 12px;
}

.tags-label {
  font-size: 12px;
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
  font-size: 11.5px;
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
  font-size: 9px;
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
  font-size: 11.5px;
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
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
}

.action-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 10px 4px;
  border-radius: var(--radius-md, 10px);
  border: 1px solid var(--border-default, var(--border-subtle));
  background: var(--bg-card, var(--bg-surface));
  cursor: pointer;
  transition: all var(--transition-fast);
}

.action-btn:hover {
  background: var(--accent-soft);
  border-color: rgba(79, 110, 247, 0.25);
}

.action-btn:hover .action-label {
  color: var(--accent);
}

.action-btn.action-active {
  background: var(--warning-soft);
  border-color: rgba(245, 166, 35, 0.2);
}

.action-btn.action-active .action-label {
  color: var(--warning);
}

.action-btn.danger:hover {
  background: var(--danger-soft);
  border-color: rgba(242, 85, 85, 0.2);
}

.action-btn.danger:hover .action-label {
  color: var(--danger);
}

.trash-actions {
  grid-template-columns: repeat(2, 1fr);
}

.action-icon {
  font-size: 18px;
}

.action-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  transition: color var(--transition-fast);
}
</style>
