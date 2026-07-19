import { onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "./useToast";
import { useConfirm } from "./useConfirm";

export interface ClipboardHotkeyOptions {
  /** Called when Escape should close the floating panel (not window mode). */
  onClose?: () => void;
  /** Allow Escape to close the panel after clearing selection. Default true. */
  allowCloseOnEscape?: boolean;
}

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

/**
 * Shared keyboard navigation for FloatingPanel and WindowApp.
 * Escape layering: search focus is handled by SearchBar (stopPropagation);
 * here we clear batch → clear selection → optionally close.
 */
export function useClipboardHotkeys(options: ClipboardHotkeyOptions = {}) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { toast } = useToast();
  const { confirm, current: confirmOpen } = useConfirm();

  async function pasteSelected(mode?: "original" | "plain") {
    const id = clipboardStore.selectedId;
    if (id == null || clipboardStore.trashFilter) return;
    const pasteMode =
      mode ??
      (settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original");
    try {
      await clipboardStore.pasteRecord(id, pasteMode);
      toast(pasteMode === "plain" ? "已粘贴为纯文本" : "已粘贴", "success");
      if (settingsStore.settings.auto_close_on_paste && options.onClose) {
        options.onClose();
      }
    } catch {
      toast("粘贴失败", "error");
    }
  }

  async function deleteSelected() {
    const id = clipboardStore.selectedId;
    if (id == null) return;
    if (clipboardStore.trashFilter) {
      const ok = await confirm({
        title: "永久删除",
        message: "确定要永久删除这条记录吗？此操作不可恢复。",
        confirmText: "永久删除",
        danger: true,
      });
      if (ok) {
        await clipboardStore.permanentlyDeleteRecord(id);
        toast("已永久删除", "success");
      }
      return;
    }
    await clipboardStore.deleteRecord(id);
    toast("已移到回收站", "success");
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
      if (options.allowCloseOnEscape !== false && options.onClose) {
        e.preventDefault();
        options.onClose();
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
      clipboardStore.selectRecord(list[nextIdx].id);
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
        if (next == null) toast("操作失败", "error");
      })();
      return;
    }

    if ((e.ctrlKey || e.metaKey) && (e.key === "t" || e.key === "T")) {
      if (clipboardStore.selectedId == null || clipboardStore.trashFilter) return;
      e.preventDefault();
      void (async () => {
        const next = await clipboardStore.togglePin(clipboardStore.selectedId!);
        if (next == null) toast("操作失败", "error");
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
