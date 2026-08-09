<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "./icons/AppIcon.vue";

/**
 * Password input with a trailing show/hide toggle.
 *
 * Defaults to masked (`type="password"`); the toggle swaps to `text` and
 * back without losing the value or focus. The caret is restored when the
 * browser allows it (masked inputs expose no selection API, hence the
 * guards). Attr fall-through works like `TextInput` — a `class` attr lands
 * on the native input so existing input classes keep their styles.
 */
defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: string;
    disabled?: boolean;
    readonly?: boolean;
  }>(),
  { disabled: false, readonly: false },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const { t } = useI18n();

const inputEl = ref<HTMLInputElement | null>(null);
const visible = ref(false);

/** No toggle on non-editable fields — matches the clear-button rule. */
const showToggle = computed(() => !props.disabled && !props.readonly);

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
}

function toggleVisibility() {
  const el = inputEl.value;
  // Masked inputs throw on selection access in real browsers — guard the read.
  let start: number | null = null;
  let end: number | null = null;
  if (el) {
    try {
      start = el.selectionStart;
      end = el.selectionEnd;
    } catch {
      /* no selection API for the current type */
    }
  }
  visible.value = !visible.value;
  nextTick(() => {
    if (!el) return;
    el.focus();
    // Only unmasked text accepts setSelectionRange.
    if (visible.value && start != null) {
      try {
        el.setSelectionRange(start, end ?? start);
      } catch {
        /* best-effort caret restore */
      }
    }
  });
}

function focus() {
  inputEl.value?.focus();
}

defineExpose({ inputEl, focus });
</script>

<template>
  <div class="input-shell">
    <input
      ref="inputEl"
      class="shell-input"
      :class="{ 'has-trailing': showToggle }"
      :type="visible ? 'text' : 'password'"
      :value="modelValue"
      :disabled="disabled"
      :readonly="readonly"
      v-bind="$attrs"
      @input="onInput"
    />
    <button
      v-if="showToggle"
      type="button"
      class="input-trailing-btn"
      :aria-label="visible ? t('common.hidePassword') : t('common.showPassword')"
      :aria-pressed="visible"
      @mousedown.prevent
      @click="toggleVisibility"
    >
      <AppIcon :name="visible ? 'eyeOff' : 'eye'" :size="14" />
    </button>
  </div>
</template>

<style scoped>
.input-shell {
  position: relative;
  display: flex;
  width: 100%;
  min-width: 0;
  flex: 1 1 auto;
}

.shell-input {
  width: 100%;
  min-width: 0;
}

/* Reserve room for the trailing button without hiding typed content.
   Two classes + scoped attr beat single-class global input padding rules. */
.shell-input.has-trailing {
  padding-right: 32px;
}

.input-trailing-btn {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 22px;
  height: 22px;
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.input-trailing-btn:hover {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.input-trailing-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
</style>
