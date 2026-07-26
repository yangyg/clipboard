<template>
  <BaseDialog :open="visible" @close="$emit('close')">
    <div class="dialog-header">
      <span class="dialog-title">{{ hasAlias ? "编辑别名" : "设置别名" }}</span>
      <button type="button" class="dialog-close" aria-label="关闭" @click="$emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </div>
    <div class="dialog-body">
      <label class="field-label" for="alias-input">别名</label>
      <input
        id="alias-input"
        ref="aliasInput"
        v-model="draft"
        class="field-input"
        type="text"
        maxlength="80"
        placeholder="方便辨认的短名称（清空则删除）"
        @keydown.enter="confirm"
      />
      <p class="field-hint">不改变粘贴内容，最多 80 字</p>
    </div>
    <div class="dialog-footer">
      <button type="button" class="btn-cancel" @click="$emit('close')">取消</button>
      <button type="button" class="btn-confirm" @click="confirm">保存</button>
    </div>
  </BaseDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useToast } from "../composables/useToast";
import AppIcon from "./icons/AppIcon.vue";
import BaseDialog from "./BaseDialog.vue";

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
    toast("保存别名失败", "error");
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
  padding: 16px 18px 0;
}

.dialog-title {
  font-size: 0.938rem;
  font-weight: 700;
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
  padding: 14px 18px 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--text-secondary);
}

.field-input {
  width: 100%;
  box-sizing: border-box;
  height: 36px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-default);
  background: var(--bg-input);
  color: var(--text-primary);
  font-family: inherit;
  font-size: 0.813rem;
}

.field-input:focus {
  outline: none;
  border-color: var(--border-focus);
}

.field-hint {
  margin: 0;
  font-size: 0.688rem;
  color: var(--text-tertiary);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px 16px;
}

.btn-cancel,
.btn-confirm {
  height: 32px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 0.813rem;
  font-weight: 600;
  cursor: pointer;
  border: 1px solid var(--border-default);
}

.btn-cancel {
  background: transparent;
  color: var(--text-secondary);
}

.btn-cancel:hover {
  background: var(--bg-hover);
}

.btn-confirm {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.btn-confirm:hover {
  background: var(--accent-hover);
}
</style>
