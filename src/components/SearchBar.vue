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
    <div class="search-trailing">
      <span
        v-if="!query"
        class="kbd search-kbd"
        :class="{ dimmed: isFocused }"
        aria-hidden="true"
      >{{ searchHint }}</span>
      <span
        v-else-if="clipboardStore.isSearching"
        class="loading-spinner small search-spinner"
        role="status"
        :aria-label="$t('search.searching')"
      ></span>
      <Transition name="fade-instant">
        <button
          v-if="query && !clipboardStore.isSearching"
          type="button"
          class="clear-btn"
          :aria-label="$t('search.clear')"
          @click="clearSearch"
        ><AppIcon name="close" :size="11" /></button>
      </Transition>
    </div>
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
  // Keep focus so keyboard users can continue typing immediately
  inputRef.value?.focus();
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
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.search-row.compact {
  max-width: 500px;
  min-width: 200px;
  flex-shrink: 1;
}

.search-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  color: var(--text-tertiary);
  pointer-events: none;
  z-index: 1;
  transition: color var(--transition-fast);
}

.search-row.focused .search-icon {
  color: var(--accent-text);
}

.search-box {
  flex: 1;
  width: 100%;
  height: 32px;
  padding: 0 40px 0 34px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  color: var(--text-primary);
  outline: none;
  transition:
    border-color var(--transition-fast),
    background var(--transition-fast),
    box-shadow var(--transition-fast);
}

.search-box:hover {
  border-color: var(--border-default);
}

.search-box:focus {
  border-color: var(--accent);
  background: var(--bg-surface);
  box-shadow: 0 0 0 2px var(--accent-soft);
  outline: none;
}

.search-box:focus-visible {
  outline: none;
}

.search-box::placeholder {
  color: var(--text-tertiary);
}

.search-row.compact .search-box {
  height: 28px;
  font-size: var(--text-md);
  padding: 0 36px 0 32px;
}

.search-row.compact .search-icon {
  left: 10px;
}

.search-trailing {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 20px;
  z-index: 1;
}

.search-row.compact .search-trailing {
  right: 6px;
}

.search-kbd {
  pointer-events: none;
  transition: opacity var(--transition-fast);
}

.search-kbd.dimmed {
  opacity: 0.4;
}

.search-spinner {
  flex-shrink: 0;
}

.clear-btn {
  width: 18px;
  height: 18px;
  border: none;
  border-radius: var(--radius-pill);
  background: var(--bg-active);
  color: var(--text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    background var(--transition-fast),
    color var(--transition-fast);
}

.clear-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.clear-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
</style>
