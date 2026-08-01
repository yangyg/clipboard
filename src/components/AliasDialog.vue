<template>
  <BaseDialog :open="visible" @close="$emit('close')">
    <div class="dialog-header">
      <span class="dialog-title">{{ hasAlias ? $t('alias.editTitle') : $t('alias.setTitle') }}</span>
      <button type="button" class="dialog-close" :aria-label="$t('common.close')" @click="$emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </div>
    <div class="dialog-body">
      <label class="field-label" for="alias-input">{{ $t('alias.label') }}</label>
      <input
        id="alias-input"
        ref="aliasInput"
        v-model="draft"
        class="field-input"
        type="text"
        maxlength="80"
        :placeholder="$t('alias.placeholder')"
        @keydown.enter="confirm"
      />
      <p class="field-hint">{{ $t('alias.hint') }}</p>
    </div>
    <div class="dialog-footer">
      <button type="button" class="btn btn-secondary btn-lg" @click="$emit('close')">{{ $t('common.cancel') }}</button>
      <button type="button" class="btn btn-primary btn-lg" @click="confirm">{{ $t('common.save') }}</button>
    </div>
  </BaseDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useToast } from "../composables/useToast";
import AppIcon from "./icons/AppIcon.vue";
import BaseDialog from "./BaseDialog.vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  visible: boolean;
  recordId: number | null;
  initialAlias?: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "saved", alias: string): void;
}>();

const clipboardStore = useClipboardStore();
const { toast } = useToast();
const { t } = useI18n();

const draft = ref("");
const aliasInput = ref<HTMLInputElement | null>(null);

const hasAlias = computed(() => !!(props.initialAlias?.trim()));

watch(
  () => props.visible,
  async (open) => {
    if (!open) return;
    draft.value = props.initialAlias?.trim() ?? "";
    await nextTick();
    aliasInput.value?.focus();
    aliasInput.value?.select();
  },
);

async function confirm() {
  if (props.recordId == null) return;
  const saved = await clipboardStore.setAlias(props.recordId, draft.value);
  if (saved === null) {
    toast(t('alias.saveFailed'), "error");
    return;
  }
  emit("saved", saved);
  emit("close");
}
</script>

<style scoped>
.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-4) 0;
}

.dialog-title {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--text-primary);
}

.dialog-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.dialog-close:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dialog-body {
  padding: var(--space-3) var(--space-4) var(--space-2);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.field-label {
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-secondary);
}

.field-input {
  width: 100%;
  box-sizing: border-box;
  height: 36px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-default);
  background: var(--bg-input);
  color: var(--text-primary);
  font-family: inherit;
  font-size: var(--text-base);
}

.field-input:focus {
  outline: none;
  border-color: var(--border-focus);
}

.field-hint {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4) var(--space-4);
}

</style>
