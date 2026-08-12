<template>
  <div class="preview-content">
    <template v-if="record.content_type === 'text'">
      <div v-if="showHtmlPreview" class="content-box html-preview" v-html="sanitizedHtml" />
      <div v-else-if="clipboardColor" class="color-preview-card">
        <div class="color-swatch-lg" :style="{ background: clipboardColor }" :title="clipboardColor" aria-hidden="true" />
        <div class="content-box color-value" v-html="plainContentHtml"></div>
      </div>
      <div v-else class="content-box" v-html="plainContentHtml"></div>
    </template>

    <template v-else-if="record.content_type === 'code'">
      <pre class="content-box code-box" v-html="plainContentHtml"></pre>
    </template>

    <template v-else-if="record.content_type === 'link'">
      <div class="link-card">
        <div class="link-icon"><AppIcon name="link" :size="22" /></div>
        <div class="link-title">{{ linkTitle }}</div>
        <!-- http(s) rendered as <a href> (right-click copy keeps working), but the
             click is intercepted and routed through open_url so the system default
             browser opens it — target=_blank navigation is unreliable in WebView2. -->
        <a
          v-if="safeLinkHref"
          class="link-url"
          :href="safeLinkHref"
          :title="$t('preview.clickToOpenLink')"
          @click.prevent="emit('open-link')"
          v-html="plainContentHtml"
        ></a>
        <button v-else-if="openableLinkUrl" type="button" class="link-url link-url-btn" :title="$t('preview.clickToOpenLink')" @click.stop="emit('open-link')">
          <span v-html="plainContentHtml"></span>
        </button>
        <div v-else class="link-url" v-html="plainContentHtml"></div>
      </div>
    </template>

    <template v-else-if="record.content_type === 'file'">
      <div class="file-card">
        <div class="file-icon"><AppIcon name="file" :size="22" /></div>
        <div class="file-path" v-html="plainContentHtml"></div>
      </div>
    </template>

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
          @click.stop="emit('open-image')"
          @keydown.enter.prevent="emit('open-image')"
          @keydown.space.prevent="emit('open-image')"
        />
        <div v-else class="image-placeholder"><AppIcon name="image" :size="28" /> {{ $t('preview.noImageData') }}</div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { ClipboardRecord } from "../types";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  record: ClipboardRecord;
  showHtmlPreview: boolean;
  sanitizedHtml: string;
  clipboardColor: string | null;
  plainContentHtml: string;
  safeLinkHref: string | null;
  openableLinkUrl: string | null;
  linkTitle: string;
  imageSrc: string | null;
}>();

const emit = defineEmits<{
  "open-image": [];
  "open-link": [];
}>();
</script>

<style scoped>
.preview-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: var(--space-4) var(--space-5);
  overflow-x: hidden;
  overflow-y: auto;
}

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

.html-preview :deep(img) { max-width: 100% !important; height: auto !important; }
.html-preview :deep(pre), .html-preview :deep(code) { white-space: pre-wrap !important; }
.html-preview :deep(table) { width: 100% !important; table-layout: fixed; border-collapse: collapse; }

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

.color-preview-card { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; }
.color-swatch-lg { width: 100%; height: 96px; border-radius: var(--radius-md, 10px); border: 1px solid var(--border-subtle); box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 12%, transparent); flex-shrink: 0; }
.color-value { font-family: var(--font-mono); font-size: var(--text-lg); letter-spacing: 0.02em; padding: 0; }

.code-box { font-family: var(--font-mono); font-size: var(--text-md); line-height: 1.6; background: var(--code-bg); color: var(--text-primary); border: none; border-radius: var(--radius-md, 10px); padding: var(--space-3) var(--space-4); white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
.link-card { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-2); padding: var(--space-3) var(--space-4); border: none; border-radius: var(--radius-md, 10px); background: var(--bg-elevated); }
.link-icon { font-size: 1.375rem; opacity: 0.8; }
.link-title { font-size: var(--text-base); font-weight: 600; color: var(--text-primary); }
.link-url { color: var(--accent-text); font-size: var(--text-md); word-break: break-all; text-decoration: none; }
.link-url:hover { text-decoration: underline; }
button.link-url-btn { display: inline; margin: 0; padding: 0; border: none; background: none; font: inherit; text-align: left; cursor: pointer; }
button.link-url-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: var(--radius-xs); }
.file-card { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border-radius: var(--radius-md, 10px); border: none; background: var(--bg-elevated); }
.file-icon { width: 36px; height: 36px; border-radius: var(--radius-sm); display: flex; align-items: center; justify-content: center; background: color-mix(in srgb, var(--type-file) 15%, transparent); color: var(--type-file); }
.file-path { font-family: var(--font-mono); font-size: var(--text-md); color: var(--text-secondary); word-break: break-all; }
.image-card { display: flex; flex-direction: column; align-items: center; gap: var(--space-3); }
.image-placeholder { padding: var(--space-5); background: var(--bg-elevated); border-radius: var(--radius-md, 10px); font-size: var(--text-3xl); opacity: 0.5; }
.image-card img, .image-thumb { max-width: 100%; border-radius: var(--radius-md, 10px); border: none; box-shadow: 0 0 0 1px var(--border-subtle); cursor: zoom-in; }
</style>
