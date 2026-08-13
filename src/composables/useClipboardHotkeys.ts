import { onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "./useToast";
import { useConfirm } from "./useConfirm";

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

/**
 * Shared keyboard navigation for the window app.
 * Escape layering: search focus is handled by SearchBar (stopPropagation);
 * here we clear batch → clear selection.
 */
export function useClipboardHotkeys() {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { toast } = useToast();
  const { confirm, current: confirmOpen } = useConfirm();
  const { t } = useI18n();

  async function pasteSelected(mode?: "original" | "plain") {
    const id = clipboardStore.selectedId;
    if (id == null || clipboardStore.trashFilter) return;
    const pasteMode =
      mode ??
      (settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original");
    try {
      await clipboardStore.pasteRecord(id, pasteMode);
      toast(pasteMode === "plain" ? t("record.pastedPlain") : t("record.pasted"), "success");
      // Window minimize on paste is handled in Rust (focus restore). Don't double-hide here.
    } catch {
      toast(t("record.pasteFailed"), "error");
    }
  }

  async function deleteSelected() {
    const id = clipboardStore.selectedId;
    if (id == null) return;
    if (clipboardStore.trashFilter) {
      const ok = await confirm({
        title: t("record.permanentDelete"),
        message: t("record.permanentDeleteMsg"),
        confirmText: t("record.permanentDelete"),
        danger: true,
      });
      if (!ok) return;
      try {
        await clipboardStore.permanentlyDeleteRecord(id);
        toast(t("record.deletedPermanently"), "success");
      } catch {
        toast(t("common.operationFailed"), "error");
      }
      return;
    }
    try {
      await clipboardStore.deleteRecord(id);
      toast(t("record.deleted"), "success");
    } catch {
      toast(t("common.operationFailed"), "error");
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (confirmOpen.value) return;

    // Let SearchBar / inputs own Escape and typing shortcuts
    if (isTypingTarget(e.target) && e.key !== "Escape") return;

    if (e.key === "Escape") {
      if (clipboardStore.batchMode) {
        e.preventDefault();
        clipboardStore.toggleBatchMode();
        return;
      }
      if (clipboardStore.selectedId !== null) {
        e.preventDefault();
        clipboardStore.clearSelection();
        return;
      }
      return;
    }

    if (isTypingTarget(e.target)) return;

    const list = clipboardStore.filteredRecords;

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!list.length) return;
      const currentIdx = list.findIndex((r) => r.id === clipboardStore.selectedId);
      let nextIdx =
        e.key === "ArrowDown"
          ? Math.min(currentIdx + 1, list.length - 1)
          : Math.max(currentIdx - 1, 0);
      if (currentIdx === -1) nextIdx = 0;
      const nextId = list[nextIdx].id;
      clipboardStore.selectRecord(nextId);
      // Move real focus so aria-activedescendant + screen readers stay in sync
      requestAnimationFrame(() => {
        document.getElementById(`record-option-${nextId}`)?.focus({ preventScroll: true });
      });
      return;
    }

    if (e.key === "Enter" && !e.altKey && !e.ctrlKey && !e.metaKey) {
      if (clipboardStore.selectedId == null) return;
      e.preventDefault();
      if (clipboardStore.trashFilter) {
        void clipboardStore.restoreRecord(clipboardStore.selectedId);
      } else {
        void pasteSelected();
      }
      return;
    }

    if (e.altKey && (e.key === "v" || e.key === "V")) {
      e.preventDefault();
      void pasteSelected("plain");
      return;
    }

    if ((e.ctrlKey || e.metaKey) && (e.key === "d" || e.key === "D")) {
      if (clipboardStore.selectedId == null || clipboardStore.trashFilter) return;
      e.preventDefault();
      void (async () => {
        const next = await clipboardStore.toggleFavorite(clipboardStore.selectedId!);
        if (next == null) toast(t("common.operationFailed"), "error");
      })();
      return;
    }

    if ((e.ctrlKey || e.metaKey) && (e.key === "t" || e.key === "T")) {
      if (clipboardStore.selectedId == null || clipboardStore.trashFilter) return;
      e.preventDefault();
      void (async () => {
        const next = await clipboardStore.togglePin(clipboardStore.selectedId!);
        if (next == null) toast(t("common.operationFailed"), "error");
      })();
      return;
    }

    if (e.key === "Delete" || e.key === "Backspace") {
      if (clipboardStore.selectedId == null) return;
      // Avoid deleting while editing; already guarded by isTypingTarget
      e.preventDefault();
      void deleteSelected();
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeyDown));
  onUnmounted(() => window.removeEventListener("keydown", onKeyDown));

  return { pasteSelected, deleteSelected };
}
