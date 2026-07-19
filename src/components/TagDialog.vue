<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="dialog-overlay" @click.self="$emit('close')">
        <div class="dialog-card">
          <!-- Header -->
          <div class="dialog-header">
            <span class="dialog-title">{{ dialogTitle }}</span>
            <button class="dialog-close" @click="$emit('close')"><AppIcon name="close" :size="14" /></button>
          </div>

          <!-- Create / Edit Mode -->
          <template v-if="mode === 'create' || mode === 'edit'">
            <div class="dialog-body">
              <label class="field-label">标签名称</label>
              <input
                ref="nameInput"
                v-model="tagName"
                class="field-input"
                type="text"
                placeholder="输入标签名称…"
                maxlength="20"
                @keydown.enter="confirmForm"
              />
              <label class="field-label">颜色</label>
              <div class="color-grid">
                <button
                  v-for="c in presetColors"
                  :key="c"
                  class="color-swatch"
                  :class="{ selected: selectedColor === c }"
                  :style="{ background: c }"
                  @click="selectedColor = c"
                >
                  <span v-if="selectedColor === c" class="swatch-check">✓</span>
                </button>
              </div>
            </div>
            <div class="dialog-footer">
              <button class="btn-cancel" @click="$emit('close')">取消</button>
              <button
                class="btn-confirm"
                :disabled="!tagName.trim()"
                @click="confirmForm"
              >{{ mode === 'edit' ? '保存' : '创建' }}</button>
            </div>
          </template>

          <!-- Assign Mode -->
          <template v-else>
            <div class="dialog-body assign-body">
              <div v-if="availableTags.length === 0" class="assign-empty">
                <p>暂无可用标签</p>
                <button class="btn-create-inline" @click="$emit('switchToCreate')">新建标签</button>
              </div>
              <label
                v-for="tag in availableTags"
                :key="tag.id"
                class="assign-item"
                :class="{ checked: assignedIds.has(tag.id) }"
              >
                <span class="assign-dot" :style="{ background: tag.color }"></span>
                <span class="assign-name">{{ tag.name }}</span>
                <span class="assign-check">
                  <span v-if="assignedIds.has(tag.id)">✓</span>
                </span>
                <input
                  type="checkbox"
                  :checked="assignedIds.has(tag.id)"
                  @change="toggleTag(tag.id)"
                  hidden
                />
              </label>
            </div>
            <div class="dialog-footer">
              <button class="btn-cancel" @click="$emit('close')">取消</button>
              <button class="btn-confirm" @click="confirmAssign">确定</button>
            </div>
          </template>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import AppIcon from "./icons/AppIcon.vue";
import type { Tag } from "../types";

const props = defineProps<{
  visible: boolean;
  mode: "create" | "assign" | "edit";
  recordId?: number;
  editTag?: Tag | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "switchToCreate"): void;
  (e: "created"): void;
  (e: "assigned"): void;
  (e: "updated"): void;
}>();

const clipboardStore = useClipboardStore();

const presetColors = [
  "#6366f1", "#7c5cfc", "#a78bfa", "#ec4899",
  "#f43f5e", "#f97316", "#eab308", "#22c55e",
  "#14b8a6", "#06b6d4", "#3b82f6", "#71717a",
];

const tagName = ref("");
const selectedColor = ref(presetColors[0]);
const assignedIds = ref<Set<number>>(new Set());
const nameInput = ref<HTMLInputElement | null>(null);

const availableTags = computed(() => clipboardStore.tags);
const dialogTitle = computed(() => {
  if (props.mode === "edit") return "编辑标签";
  if (props.mode === "create") return "新建标签";
  return "添加标签";
});

// Reset form when dialog opens
watch(() => props.visible, async (v) => {
  if (v) {
    assignedIds.value = new Set();
    await clipboardStore.loadTags();
    if (props.mode === "edit" && props.editTag) {
      tagName.value = props.editTag.name;
      selectedColor.value = props.editTag.color;
    } else {
      tagName.value = "";
      selectedColor.value = presetColors[0];
    }
    // Pre-select tags already on record
    if (props.mode === "assign" && props.recordId) {
      const record = clipboardStore.records.find((r) => r.id === props.recordId);
      const recordTagNames = record?.tags ?? [];
      const next = new Set<number>();
      for (const tag of availableTags.value) {
        if (recordTagNames.includes(tag.name)) next.add(tag.id);
      }
      assignedIds.value = next;
    }
    await nextTick();
    nameInput.value?.focus();
  }
});

function toggleTag(tagId: number) {
  const next = new Set(assignedIds.value);
  if (next.has(tagId)) next.delete(tagId);
  else next.add(tagId);
  assignedIds.value = next;
}

async function confirmForm() {
  const name = tagName.value.trim();
  if (!name) return;
  try {
    if (props.mode === "edit") {
      if (!props.editTag) return;
      await clipboardStore.updateTag(props.editTag.id, name, selectedColor.value);
      emit("updated");
      emit("close");
      return;
    }
    await clipboardStore.createTag(name, selectedColor.value);
    emit("created");
    // Assign flow (has recordId): stay open so parent can return to assign mode
    if (props.recordId != null) return;
    emit("close");
  } catch {
    // parent / caller may toast; keep dialog open for retry
  }
}

async function confirmAssign() {
  if (!props.recordId) return;
  const record = clipboardStore.records.find((r) => r.id === props.recordId);
  const recordTagNames = record?.tags ?? [];

  // Remove unselected tags
  for (const tag of availableTags.value) {
    if (recordTagNames.includes(tag.name) && !assignedIds.value.has(tag.id)) {
      await clipboardStore.removeTagFromRecord(props.recordId, tag.id, tag.name);
    }
    if (!recordTagNames.includes(tag.name) && assignedIds.value.has(tag.id)) {
      await clipboardStore.addTagToRecord(props.recordId, tag.id, tag.name);
    }
  }
  emit("assigned");
  emit("close");
}
</script>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog-card {
  width: 340px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg, 14px);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.dialog-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
}

.dialog-close {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-close:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dialog-body {
  padding: 16px;
}

.assign-body {
  max-height: 320px;
  overflow-y: auto;
}

.field-label {
  display: block;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-secondary);
  margin-bottom: 6px;
  margin-top: 12px;
}

.field-label:first-child {
  margin-top: 0;
}

.field-input {
  width: 100%;
  height: 36px;
  padding: 0 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  outline: none;
  transition: border-color var(--transition-fast);
}

.field-input:focus {
  border-color: var(--accent);
}

.color-grid {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 8px;
  margin-top: 6px;
}

.color-swatch {
  width: 100%;
  aspect-ratio: 1;
  border-radius: var(--radius-sm);
  border: 2px solid transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: border-color var(--transition-fast), transform var(--transition-fast);
}

.color-swatch:hover {
  transform: scale(1.08);
}

.color-swatch.selected {
  border-color: var(--text-primary);
}

.swatch-check {
  color: #fff;
  font-size: 14px;
  font-weight: 700;
  text-shadow: 0 1px 2px rgba(0,0,0,0.3);
}

/* Assign mode items */
.assign-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.assign-item:hover {
  background: var(--bg-hover);
}

.assign-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.assign-name {
  flex: 1;
  font-size: 13px;
  color: var(--text-primary);
}

.assign-check {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  border: 1.5px solid var(--border-default, var(--text-tertiary));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: transparent;
  flex-shrink: 0;
  transition: all var(--transition-fast);
}

.assign-item.checked .assign-check {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.assign-empty {
  padding: 20px 0;
  text-align: center;
  font-size: 13px;
  color: var(--text-tertiary);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.btn-create-inline {
  height: 30px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  font-weight: 500;
  background: var(--accent);
  color: #fff;
  cursor: pointer;
}

.btn-create-inline:hover {
  background: var(--accent-hover);
}

.assign-or {
  text-align: center;
  font-size: 12px;
  color: var(--text-tertiary);
  padding: 8px 0;
}

.btn-create-new {
  width: 100%;
  padding: 8px;
  border-radius: var(--radius-sm);
  border: 1px dashed var(--border-default, var(--border-subtle));
  background: transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-create-new:hover {
  color: var(--accent);
  border-color: var(--accent);
}

/* Footer */
.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn-cancel {
  height: 32px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border-subtle);
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-cancel:hover {
  background: var(--bg-hover);
}

.btn-confirm {
  height: 32px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: #fff;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-confirm:hover {
  background: var(--accent-light, #6b85fa);
}

.btn-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Modal transition */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}

.modal-enter-active .dialog-card,
.modal-leave-active .dialog-card {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .dialog-card,
.modal-leave-to .dialog-card {
  transform: scale(0.95);
  opacity: 0;
}
</style>
