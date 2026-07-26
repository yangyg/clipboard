<template>
  <div class="search-row" :class="{ focused: isFocused, compact: compact }">
    <span class="search-icon"><AppIcon name="search" :size="14" /></span>
    <input
      ref="inputRef"
      v-model="query"
      class="search-box"
      type="text"
      :aria-label="$t('search.ariaLabel')"
      :placeholder="compact ? $t('search.placeholderCompact') : $t('search.placeholder')"
      @focus="isFocused = true"
      @blur="isFocused = false"
      @input="onInput"
      @keydown.escape.stop.prevent="onEscapeInSearch"
    />
    <span
      v-if="!query"
      class="search-kbd"
      :class="{ dimmed: isFocused }"
      aria-hidden="true"
    >{{ searchHint }}</span>
    <span v-if="clipboardStore.isSearching" class="search-spinner" :aria-label="$t('search.searching')"></span>
    <Transition name="fade-instant">
      <button
        v-if="query"
        type="button"
        class="clear-btn"
        :aria-label="$t('search.clear')"
        @click="clearSearch"
      ><AppIcon name="close" :size="11" /></button>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  compact?: boolean;
}>();

const clipboardStore = useClipboardStore();
const inputRef = ref<HTMLInputElement | null>(null);
const query = ref("");
const isFocused = ref(false);

const isMac = computed(() => /Mac|iPhone|iPad/.test(navigator.platform));
const searchHint = computed(() => (isMac.value ? "⌘K" : "Ctrl+K"));

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

function onInput() {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    clipboardStore.search(query.value);
  }, 250);
}

function clearSearch() {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  query.value = "";
  clipboardStore.search("");
  inputRef.value?.blur();
}

function onEscapeInSearch() {
  if (query.value) {
    clearSearch();
  } else {
    inputRef.value?.blur();
  }
}

function onGlobalKey(e: KeyboardEvent) {
  if ((e.key === "/" || ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K"))) && !isFocused.value) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    e.preventDefault();
    inputRef.value?.focus();
  }
}

watch(
  () => clipboardStore.searchQuery,
  (val) => {
    if (!val && query.value) {
      query.value = "";
    }
  }
);

onMounted(() => {
  window.addEventListener("keydown", onGlobalKey);
});

onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  window.removeEventListener("keydown", onGlobalKey);
});
</script>

<style scoped>
.search-row {
  display: flex;
  align-items: center;
  gap: 8px;
  position: relative;
}

.search-row.compact {
  width: 100%;
  max-width: 500px;
  min-width: 200px;
  flex-shrink: 1;
}

.search-row.compact .search-box {
  height: 28px;
  font-size: 0.78rem;
  background: var(--bg-surface);
  border-color: var(--border-default);
  border-radius: var(--radius-md);
  padding: 0 12px 0 32px;
}

.search-row.compact .search-box:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-softer);
}

.search-row.compact .search-icon {
  left: 14px;
  font-size: 0.75rem;
}

.search-row.compact .search-kbd {
  right: 6px;
  font-size: 0.625rem;
}

.search-icon {
  position: absolute;
  left: 26px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.81rem;
  color: var(--text-tertiary);
  pointer-events: none;
  z-index: 1;
  transition: color var(--transition-fast);
}

.search-row.focused .search-icon {
  color: var(--accent);
}

.search-box {
  flex: 1;
  height: 36px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 12px 0 34px;
  font-size: 0.81rem;
  color: var(--text-primary);
  transition: border-color var(--transition-fast), background var(--transition-smooth);
}

.search-box:focus {
  border-color: var(--border-focus);
  background: var(--bg-surface);
}

.search-box::placeholder {
  color: var(--text-tertiary);
}

.search-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-default);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
  flex-shrink: 0;
}

.search-kbd {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  font-family: var(--font-mono);
  font-size: 0.625rem;
  color: var(--text-tertiary);
  background: var(--bg-active);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
  padding: 1px 6px;
  pointer-events: none;
  transition: opacity var(--transition-fast);
}

.search-kbd.dimmed {
  opacity: 0.4;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.clear-btn {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--bg-active);
  color: var(--text-tertiary);
  font-size: 0.625rem;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: all var(--transition-fast);
}

.clear-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
