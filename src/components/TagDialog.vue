<template>
  <BaseDialog :open="visible" @close="$emit('close')">
    <div class="dialog-header">
      <span class="dialog-title">{{ dialogTitle }}</span>
      <button type="button" class="dialog-close" aria-label="关闭" @click="$emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </div>

    <template v-if="mode === 'create' || mode === 'edit'">
      <div class="dialog-body">
        <label class="field-label" for="tag-name-input">标签名称</label>
        <input
          id="tag-name-input"
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
            type="button"
            class="color-swatch"
            :class="{ selected: selectedColor === c }"
            :style="{ background: c }"
            :aria-label="`颜色 ${c}`"
            @click="selectedColor = c"
          >
            <span v-if="selectedColor === c" class="swatch-check">✓</span>
          </button>
        </div>
      </div>
      <div class="dialog-footer">
        <button type="button" class="btn-cancel" @click="$emit('close')">取消</button>
        <button
          type="button"
          class="btn-confirm"
          :disabled="!tagName.trim()"
          @click="confirmForm"
        >{{ mode === 'edit' ? '保存' : '创建' }}</button>
      </div>
    </template>

    <template v-else>
      <div class="dialog-body assign-body">
        <div v-if="availableTags.length === 0" class="assign-empty">
          <p>暂无可用标签</p>
          <button type="button" class="btn-create-inline" @click="$emit('switchToCreate')">新建标签</button>
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
        <button type="button" class="btn-cancel" @click="$emit('close')">取消</button>
        <button type="button" class="btn-confirm" @click="confirmAssign">确定</button>
      </div>
    </template>
  </BaseDialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useToast } from "../composables/useToast";
import { resolveTagPalette } from "../utils/themeColors";
import AppIcon from "./icons/AppIcon.vue";
import BaseDialog from "./BaseDialog.vue";
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
const { toast } = useToast();

const presetColors = ref(resolveTagPalette());
const tagName = ref("");
const selectedColor = ref(presetColors.value[0] ?? "#6366f1");
const assignedIds = ref<Set<number>>(new Set());
const nameInput = ref<HTMLInputElement | null>(null);

const availableTags = computed(() => clipboardStore.tags);
const dialogTitle = computed(() => {
  if (props.mode === "edit") return "编辑标签";
  if (props.mode === "create") return "新建标签";
  return "添加标签";
});

watch(() => props.visible, async (v) => {
  if (v) {
    assignedIds.value = new Set();
    await clipboardStore.loadTags();
    const existingColors = clipboardStore.tags.map((t) => t.color);
    if (props.mode === "edit" && props.editTag?.color) {
      existingColors.unshift(props.editTag.color);
    }
    presetColors.value = resolveTagPalette(existingColors);
    if (props.mode === "edit" && props.editTag) {
      tagName.value = props.editTag.name;
      selectedColor.value = props.editTag.color;
    } else {
      tagName.value = "";
      selectedColor.value = presetColors.value[0] ?? "#6366f1";
    }
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
    if (props.recordId != null) return;
    emit("close");
  } catch (e) {
    toast(props.mode === "edit" ? "保存标签失败" : "创建标签失败", "error");
    console.error("Tag form failed:", e);
  }
}

async function confirmAssign() {
  if (!props.recordId) return;
  const selected = availableTags.value.filter((t) => assignedIds.value.has(t.id));
  try {
    await clipboardStore.setRecordTags(
      props.recordId,
      selected.map((t) => t.id),
      selected.map((t) => t.name),
    );
    emit("assigned");
    emit("close");
  } catch (e) {
    toast("设置标签失败", "error");
    console.error("Assign tags failed:", e);
  }
}
</script>

<style scoped>
.assign-body {
  max-height: 320px;
  overflow-y: auto;
}

.field-label {
  display: block;
  font-size: var(--text-sm);
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
  font-size: var(--text-base);
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
  font-size: var(--text-lg);
  font-weight: 700;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

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
  font-size: var(--text-base);
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
  font-size: var(--text-xs);
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
  font-size: var(--text-base);
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
  font-size: var(--text-md);
  font-weight: 500;
  background: var(--accent);
  color: #fff;
  cursor: pointer;
}

.btn-create-inline:hover {
  background: var(--accent-hover);
}
</style>
