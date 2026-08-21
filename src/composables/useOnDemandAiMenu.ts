import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { onDemandAiActions, type AiEnrichMode } from "../utils/aiEnrich";
import type { ClipboardRecord } from "../types";
import type { ContextMenuItem } from "../components/ContextMenu.vue";

export function useOnDemandAiMenu(getRecord: () => ClipboardRecord) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { t } = useI18n();
  const aiMenuAnchorEl = ref<HTMLElement | null>(null);
  const aiMenu = reactive({ visible: false, x: 0, y: 0 });

  const aiActions = computed(() => onDemandAiActions(getRecord(), settingsStore.settings));
  const aiBusy = computed(() => clipboardStore.aiBusyId === getRecord().id);

  const aiMenuItems = computed<ContextMenuItem[]>(() =>
    aiActions.value.map((id) => ({
      id,
      label: t(id === "summary" ? "record.aiSummary" : "record.aiTags"),
      icon: id === "summary" ? "sparkles" : "tag",
    })),
  );

  function toggleAiMenu(e: MouseEvent) {
    if (aiBusy.value) return;
    if (aiMenu.visible) {
      aiMenu.visible = false;
      return;
    }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    aiMenu.x = rect.left;
    aiMenu.y = rect.bottom;
    aiMenu.visible = true;
  }

  function closeAiMenu() {
    aiMenu.visible = false;
  }

  async function onAiMenuSelect(id: string) {
    if (id !== "summary" && id !== "tags") return;
    await clipboardStore.enrichRecord(getRecord().id, id as AiEnrichMode);
  }

  return {
    aiMenuAnchorEl,
    aiMenu,
    aiActions,
    aiBusy,
    aiMenuItems,
    toggleAiMenu,
    closeAiMenu,
    onAiMenuSelect,
  };
}
