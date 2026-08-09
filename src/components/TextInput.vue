<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "./icons/AppIcon.vue";

/**
 * Single-line text input with a trailing clear button.
 *
 * The clear button shows only while the field is editable (not disabled /
 * readonly) and non-empty. Clearing emits `update:modelValue` and refocuses
 * the input so typing can continue uninterrupted. Extra attrs (placeholder,
 * maxlength, autocomplete, aria-*, keydown handlers…) fall through to the
 * native input; a `class` attr lands on the input itself so existing input
 * classes (`.auto-tag-input`, `.field-input`, …) keep their styles.
 */
defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** Text-like single-line types; `url` keeps native URL semantics. */
    type?: "text" | "url";
    disabled?: boolean;
    readonly?: boolean;
  }>(),
  { type: "text", disabled: false, readonly: false },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const { t } = useI18n();

const inputEl = ref<HTMLInputElement | null>(null);

const showClear = computed(
  () => props.modelValue !== "" && !props.disabled && !props.readonly,
);

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
}

/** Clear + refocus so the user can keep typing right away. */
function clear() {
  emit("update:modelValue", "");
  inputEl.value?.focus();
}

function focus() {
  inputEl.value?.focus();
}

function select() {
  inputEl.value?.select();
}

defineExpose({ inputEl, focus, select });
</script>

<template>
  <div class="input-shell">
    <input
      ref="inputEl"
      class="shell-input"
      :class="{ 'has-trailing': showClear }"
      :type="type"
      :value="modelValue"
      :disabled="disabled"
      :readonly="readonly"
      v-bind="$attrs"
      @input="onInput"
    />
    <button
      v-if="showClear"
      type="button"
      class="input-trailing-btn"
      :aria-label="t('common.clearInput')"
      @mousedown.prevent
      @click="clear"
    >
      <AppIcon name="close" :size="12" />
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
