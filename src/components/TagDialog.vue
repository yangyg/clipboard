<template>
  <BaseDialog :open="visible" labelled-by="tag-dialog-title" @close="$emit('close')">
    <div class="dialog-header">
      <span id="tag-dialog-title" class="dialog-title">{{ dialogTitle }}</span>
      <button type="button" class="dialog-close" :aria-label="$t('common.close')" @click="$emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </div>

    <template v-if="mode === 'create' || mode === 'edit'">
      <div class="dialog-body">
        <label class="field-label" for="tag-name-input">{{ $t('tagDialog.nameLabel') }}</label>
        <TextInput
          id="tag-name-input"
          ref="nameInput"
          v-model="tagName"
          class="field-input"
          :class="{ 'field-input-error': nameDuplicate }"
          type="text"
          :placeholder="$t('tagDialog.namePlaceholder')"
          maxlength="20"
          :aria-invalid="nameDuplicate"
          @keydown.enter="confirmForm"
        />
        <p v-if="nameDuplicate" class="field-error" role="alert">{{ $t('tagDialog.nameDuplicate') }}</p>
        <label class="field-label">{{ $t('tagDialog.colorLabel') }}</label>
        <div class="color-grid">
          <button
            v-for="c in presetColors"
            :key="c"
            type="button"
            class="color-swatch"
            :class="{ selected: selectedColor === c }"
            :style="{ background: c }"
            :aria-label="`${$t('tagDialog.colorLabel')} ${c}`"
            @click="selectedColor = c"
          >
            <span v-if="selectedColor === c" class="swatch-check">✓</span>
          </button>
        </div>
      </div>
      <div class="dialog-footer">
        <button type="button" class="btn btn-secondary btn-lg" @click="$emit('close')">{{ $t('common.cancel') }}</button>
        <button
          type="button"
          class="btn btn-primary btn-lg"
          :disabled="!canSubmit"
          @click="confirmForm"
        >{{ mode === 'edit' ? $t('common.save') : $t('common.create') }}</button>
      </div>
    </template>

    <template v-else>
      <div class="dialog-body assign-body">
        <div v-if="availableTags.length === 0" class="assign-empty">
          <p>{{ $t('tagDialog.noTags') }}</p>
          <button type="button" class="btn btn-primary" @click="$emit('switchToCreate')">{{ $t('tagDialog.createTag') }}</button>
        </div>
        <label
          v-for="tag in availableTags"
          :key="tag.id"
          class="assign-item"
          :class="{ checked: assignedIds.has(tag.id) }"
        >
          <input
            type="checkbox"
            class="assign-checkbox"
            :checked="assignedIds.has(tag.id)"
            :aria-label="tag.name"
            @change="toggleTag(tag.id)"
          />
          <span class="assign-dot" :style="{ background: tag.color }" aria-hidden="true"></span>
          <span class="assign-name">{{ tag.name }}</span>
          <span class="assign-check" aria-hidden="true">
            <span v-if="assignedIds.has(tag.id)">✓</span>
          </span>
        </label>
      </div>
      <div class="dialog-footer">
        <button type="button" class="btn btn-secondary btn-lg" @click="$emit('close')">{{ $t('common.cancel') }}</button>
        <button type="button" class="btn btn-primary btn-lg" @click="confirmAssign">{{ $t('common.confirm') }}</button>
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
import TextInput from "./TextInput.vue";
import type { Tag } from "../types";
import { useI18n } from "vue-i18n";

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
const { t } = useI18n();

const presetColors = ref(resolveTagPalette());
const tagName = ref("");
const selectedColor = ref(presetColors.value[0] ?? "#0078d4");
const assignedIds = ref<Set<number>>(new Set());
const nameInput = ref<InstanceType<typeof TextInput> | null>(null);

const availableTags = computed(() => clipboardStore.tags);
const trimmedName = computed(() => tagName.value.trim());
// Create mode only: mirror the DB UNIQUE(name) constraint (exact, case-sensitive)
// so duplicates are flagged while typing instead of failing at submit.
const nameDuplicate = computed(
  () =>
    props.mode === "create" &&
    trimmedName.value !== "" &&
    availableTags.value.some((tag) => tag.name === trimmedName.value),
);
const canSubmit = computed(() => trimmedName.value !== "" && !nameDuplicate.value);
const dialogTitle = computed(() => {
  if (props.mode === "edit") return t('tagDialog.editTitle');
  if (props.mode === "create") return t('tagDialog.createTitle');
  return t('tagDialog.assignTitle');
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
      selectedColor.value = presetColors.value[0] ?? "#0078d4";
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
  const name = trimmedName.value;
  if (!name) return;
  // Inline duplicate guard for create mode; submit-time backend check stays as fallback.
  if (nameDuplicate.value) return;
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
    if (props.mode === "create" && String(e).includes("TAG_NAME_EXISTS")) {
      // Fallback: name raced past the inline check — refresh the tag list so
      // the inline hint catches it, and tell the user the exact reason.
      await clipboardStore.loadTags();
      toast(t('tagDialog.nameDuplicate'), "error");
    } else {
      toast(props.mode === "edit" ? t('tagDialog.saveFailed') : t('tagDialog.createFailed'), "error");
    }
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
    toast(t('tagDialog.assignFailed'), "error");
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

/* :deep — the input now lives inside the TextInput shell component. */
:deep(.field-input) {
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

:deep(.field-input:focus) {
  border-color: var(--accent);
}

:deep(.field-input-error) {
  border-color: var(--danger);
}

:deep(.field-input-error:focus) {
  border-color: var(--danger);
}

.field-error {
  margin: 6px 0 0;
  font-size: var(--text-sm);
  color: var(--danger);
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
  font-weight: 600;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.assign-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.assign-item:hover {
  background: var(--accent-softer);
}

.assign-checkbox {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  opacity: 0;
  cursor: pointer;
  z-index: 1;
}

.assign-item:focus-within {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
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
  border-radius: var(--radius-xs);
  border: 1.5px solid var(--border-default, var(--text-tertiary));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-xs);
  color: transparent;
  flex-shrink: 0;
  transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
}

.assign-item.checked .assign-check {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-on-accent);
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
</style>
