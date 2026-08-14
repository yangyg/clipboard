<template>
  <div class="search-root" :class="{ compact: compact }">
    <button
      v-if="mode === 'icon' && !expanded"
      type="button"
      class="icon-btn search-trigger"
      :class="{ 'icon-btn-md': compact }"
      :aria-label="$t('search.ariaLabel')"
      :title="searchHint"
      @click="expandAndFocus"
    ><AppIcon name="search" :size="compact ? 15 : 16" /></button>

    <div
      v-show="mode === 'full' || expanded"
      class="search-row"
      :class="{ focused: isFocused, compact: compact }"
    >
      <span class="search-icon"><AppIcon name="search" :size="14" /></span>
      <input
        ref="inputRef"
        v-model="query"
        class="search-box"
        type="text"
        :aria-label="$t('search.ariaLabel')"
        aria-autocomplete="list"
        :aria-expanded="showDropdown"
        :aria-controls="showDropdown && suggestions.length > 0 ? 'search-suggest-list' : undefined"
        :aria-activedescendant="showDropdown && activeIndex >= 0 ? 'suggest-' + activeIndex : undefined"
        :placeholder="compact ? $t('search.placeholderCompact') : $t('search.placeholder')"
        @focus="onFocus"
        @blur="onBlur"
        @input="onInput"
        @keydown="onInputKeydown"
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
  </div>

  <Teleport to="body">
    <Transition name="fade-instant">
      <SearchSuggest
        v-if="showDropdown && suggestions.length > 0"
        :suggestions="suggestions"
        :active-index="activeIndex"
        :pos="pos"
        :el-ref="setDropdownEl"
        @accept="acceptSuggestion"
        @remove="removeSuggestion"
        @clear-all="clearAllHistory"
        @hover="activeIndex = $event"
      />
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, reactive, onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useSearchHistory } from "../composables/useSearchHistory";
import { highlightSearchHtml } from "../utils/highlightSearch";
import AppIcon from "./icons/AppIcon.vue";
import SearchSuggest, { type Suggestion } from "./SearchSuggest.vue";

const MAX_HISTORY = 10;

defineProps<{
  compact?: boolean;
}>();

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { history, loadHistory, recordHistory, clearHistory, removeHistory } = useSearchHistory();

/** Search bar display mode from settings (`full` | `icon` | `hidden`). */
const mode = computed(() => settingsStore.settings.search_mode);
/**
 * Whether the input is temporarily revealed in `icon` / `hidden` mode.
 * `full` mode keeps the box always shown; `expanded` is irrelevant there.
 */
const expanded = ref(false);

const inputRef = ref<HTMLInputElement | null>(null);
const dropdownRef = ref<HTMLElement | null>(null);

/** Callback ref so SearchSuggest can forward its root element up —
 * positionDropdown needs its height for flip-above detection. */
function setDropdownEl(el: unknown) {
  dropdownRef.value = el as HTMLElement | null;
}
const query = ref("");
const isFocused = ref(false);

const suggestions = ref<Suggestion[]>([]);
const activeIndex = ref(-1);
const showDropdown = ref(false);
const pos = reactive({ x: 0, y: 0, width: 260 });

const isMac = computed(() => /Mac|iPhone|iPad/.test(navigator.platform));
const searchHint = computed(() => (isMac.value ? "⌘K" : "Ctrl+K"));

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

function buildSuggestions() {
  const q = query.value.trim().toLowerCase();
  const entries = q ? history.value.filter((h) => h.toLowerCase().includes(q)) : history.value;
  suggestions.value = entries.slice(0, MAX_HISTORY).map((label) => ({
    label,
    html: highlightSearchHtml(label, q),
  }));
}

function positionDropdown() {
  const el = inputRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const pad = 8;
  const gap = 6;
  pos.width = Math.max(rect.width, 260);
  pos.x = Math.max(pad, Math.min(rect.left, window.innerWidth - pos.width - pad));
  const dd = dropdownRef.value;
  const ddHeight = dd?.offsetHeight ?? 0;
  let y = rect.bottom + gap;
  if (y + ddHeight > window.innerHeight - pad && rect.top - gap - ddHeight >= pad) {
    y = rect.top - gap - ddHeight;
  }
  pos.y = y;
}

function openDropdown() {
  showDropdown.value = true;
  nextTick(positionDropdown);
}

function closeDropdown() {
  showDropdown.value = false;
  activeIndex.value = -1;
}

function onFocus() {
  isFocused.value = true;
  buildSuggestions();
  if (suggestions.value.length > 0) openDropdown();
}

function onBlur() {
  isFocused.value = false;
  // Icon / hidden modes: collapse back once the user leaves an empty box.
  // A non-empty query keeps the box visible so the active search stays clear.
  if (mode.value !== "full" && !query.value) {
    expanded.value = false;
  }
}

/** Reveal the input (icon / hidden modes) and focus it. */
function expandAndFocus() {
  if (mode.value !== "full") expanded.value = true;
  nextTick(() => inputRef.value?.focus());
}

function onInput() {
  buildSuggestions();
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    clipboardStore.search(query.value);
  }, 250);
}

function acceptSuggestion(s: Suggestion) {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  query.value = s.label;
  recordHistory(s.label);
  closeDropdown();
  clipboardStore.search(s.label);
}

function removeSuggestion(s: Suggestion) {
  removeHistory(s.label);
  buildSuggestions();
}

function clearAllHistory() {
  clearHistory();
  buildSuggestions();
}

function onInputKeydown(e: KeyboardEvent) {
  const list = suggestions.value;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (!list.length) return;
    if (!showDropdown.value) {
      openDropdown();
      activeIndex.value = 0;
      return;
    }
    activeIndex.value = (activeIndex.value + 1) % list.length;
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (!list.length) return;
    activeIndex.value = (activeIndex.value - 1 + list.length) % list.length;
    return;
  }
  if (e.key === "Delete") {
    const item = list[activeIndex.value];
    if (showDropdown.value && item) {
      e.preventDefault();
      removeSuggestion(item);
    }
    return;
  }
  if (e.key === "Enter") {
    if (showDropdown.value && activeIndex.value >= 0 && list[activeIndex.value]) {
      e.preventDefault();
      acceptSuggestion(list[activeIndex.value]);
      return;
    }
    // Explicit submit without picking a suggestion — record history.
    if (showDropdown.value) closeDropdown();
    recordHistory(query.value);
    return;
  }
  if (e.key === "Escape") {
    if (showDropdown.value) {
      e.preventDefault();
      e.stopPropagation();
      closeDropdown();
      return;
    }
    if (query.value) {
      e.preventDefault();
      e.stopPropagation();
      clearSearch();
      return;
    }
    inputRef.value?.blur();
  }
}

function clearSearch() {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  query.value = "";
  clipboardStore.search("");
  closeDropdown();
  // Keep focus so keyboard users can continue typing immediately
  inputRef.value?.focus();
}

function onGlobalKey(e: KeyboardEvent) {
  if ((e.key === "/" || ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K"))) && !isFocused.value) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    e.preventDefault();
    expandAndFocus();
  }
}

// Keep the active row in range as suggestions change under the cursor.
watch(suggestions, (list) => {
  if (list.length === 0) {
    activeIndex.value = -1;
  } else if (activeIndex.value >= list.length) {
    activeIndex.value = list.length - 1;
  }
});

watch(
  () => clipboardStore.searchQuery,
  (val) => {
    if (!val && query.value) {
      query.value = "";
      buildSuggestions();
    }
  }
);

// Auto-open/close with focus + non-empty suggestions; reposition on resize.
watch(
  () => [isFocused.value, suggestions.value.length] as const,
  () => {
    if (isFocused.value && suggestions.value.length > 0) {
      openDropdown();
    } else {
      closeDropdown();
    }
  }
);

onMounted(() => {
  window.addEventListener("keydown", onGlobalKey);
  window.addEventListener("resize", positionDropdown);
  void loadHistory();
});

onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  window.removeEventListener("keydown", onGlobalKey);
  window.removeEventListener("resize", positionDropdown);
});
</script>

<style scoped>
.search-root {
  width: 100%;
  display: flex;
  min-width: 0;
}

.search-root.compact {
  max-width: 500px;
  min-width: 200px;
  flex-shrink: 1;
  justify-content: center;
}

/* Icon-only trigger (icon mode, collapsed) */
.search-trigger {
  flex-shrink: 0;
}

.search-row {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.search-row.compact {
  min-width: 0;
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
  background: var(--accent-soft);
  color: var(--accent-text);
}

.clear-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

/* Suggestion dropdown styles live in SearchSuggest.vue alongside its markup. */
</style>
