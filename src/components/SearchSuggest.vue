<!-- Search history suggestion dropdown. Rendered inside a body <Teleport> by
     SearchBar (escapes toolbar overflow clipping); SearchBar owns positioning
     and keyboard navigation, this component is purely presentational. -->
<script lang="ts">
export interface Suggestion {
  label: string;
  html: string;
}
</script>

<template>
  <div
    :ref="elRef"
    class="search-suggest"
    role="listbox"
    :aria-label="$t('search.suggestionsHistory')"
    :style="{ left: pos.x + 'px', top: pos.y + 'px', width: pos.width + 'px' }"
  >
    <div
      v-for="(s, i) in suggestions"
      :key="s.label"
      class="suggest-item"
      :class="{ active: i === activeIndex }"
      :id="'suggest-' + i"
      role="option"
      :aria-selected="i === activeIndex"
      @mousedown.prevent="emit('accept', s)"
      @mouseenter="emit('hover', i)"
    >
      <span class="suggest-icon"><AppIcon name="history" :size="13" /></span>
      <span class="suggest-label" v-html="s.html"></span>
      <button
        type="button"
        class="suggest-delete"
        tabindex="-1"
        :aria-label="$t('search.removeHistory')"
        @mousedown.stop.prevent="emit('remove', s)"
        @mouseenter.stop="emit('hover', i)"
      ><AppIcon name="close" :size="11" /></button>
    </div>
    <div class="suggest-footer">
      <button
        type="button"
        class="suggest-clear-all"
        tabindex="-1"
        @mousedown.prevent="emit('clear-all')"
      >{{ $t('search.clearHistory') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { VNodeRef } from "vue";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  suggestions: Suggestion[];
  activeIndex: number;
  pos: { x: number; y: number; width: number };
  /** Callback ref forwarding the root element to SearchBar for position measurement. */
  elRef: VNodeRef;
}>();

const emit = defineEmits<{
  accept: [suggestion: Suggestion];
  remove: [suggestion: Suggestion];
  "clear-all": [];
  hover: [index: number];
}>();
</script>

<style scoped>
.search-suggest {
  position: fixed;
  z-index: 1200;
  max-height: 300px;
  overflow-y: auto;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  padding: var(--space-1);
}

.suggest-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 6px 10px;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  font-size: var(--text-md);
  color: var(--text-secondary);
  text-align: left;
  cursor: pointer;
  font-family: inherit;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.suggest-item:hover,
.suggest-item.active,
.suggest-item:focus-visible {
  background: var(--accent-softer);
  color: var(--accent-text);
  outline: none;
}

.suggest-icon {
  flex-shrink: 0;
  display: flex;
  color: var(--text-tertiary);
  transition: color var(--transition-fast);
}

.suggest-item.active .suggest-icon {
  color: var(--accent-text);
}

.suggest-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.suggest-delete {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition:
    opacity var(--transition-fast),
    background var(--transition-fast),
    color var(--transition-fast);
}

.suggest-item:hover .suggest-delete,
.suggest-item.active .suggest-delete,
.suggest-delete:focus-visible {
  opacity: 1;
}

.suggest-delete:hover {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.suggest-footer {
  border-top: 1px solid var(--border-subtle);
  margin-top: var(--space-1);
  padding-top: var(--space-1);
}

.suggest-clear-all {
  width: 100%;
  padding: 6px 10px;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  font-size: var(--text-md);
  color: var(--text-tertiary);
  text-align: left;
  cursor: pointer;
  font-family: inherit;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.suggest-clear-all:hover,
.suggest-clear-all:focus-visible {
  background: var(--danger-soft);
  color: var(--danger);
  outline: none;
}
</style>
