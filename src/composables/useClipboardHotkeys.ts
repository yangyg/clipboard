import { onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "./useToast";
import { useConfirm } from "./useConfirm";
import { useExpiryGuard } from "./useExpiryGuard";
import { useBatchActions } from "./useBatchActions";
import { toastPasteOutcome } from "../utils/pasteNotify";
import { humanizeInvokeError } from "../utils/invokeError";

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

export type ListHotkeyAction =
  | "toggle-batch-select"
  | "batch-delete"
  | "select-all"
  | "paste"
  | "restore"
  | "delete"
  | "plain-paste";

/** Pure key → action map for the record list (exported for tests). */
export function resolveListHotkey(
  e: { key: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean },
  ctx: {
    batchMode: boolean;
    trashFilter: boolean;
    selectedId: number | null;
    selectedCount: number;
  },
): ListHotkeyAction | null {
  if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
    if (ctx.batchMode) return "select-all";
    return null;
  }
  if (e.key === "Enter" && !e.altKey && !e.ctrlKey && !e.metaKey) {
    if (ctx.selectedId == null) return null;
    if (ctx.batchMode) return "toggle-batch-select";
    if (ctx.trashFilter) return "restore";
    return "paste";
  }
  if (e.altKey && (e.key === "v" || e.key === "V")) {
    if (ctx.batchMode || ctx.trashFilter || ctx.selectedId == null) return null;
    return "plain-paste";
  }
  if (e.key === "Delete" || e.key === "Backspace") {
    if (ctx.batchMode && ctx.selectedCount > 0) return "batch-delete";
    if (ctx.selectedId == null) return null;
    return "delete";
  }
  return null;
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
  const { confirmUnprotectIfNeeded } = useExpiryGuard();
  const { t } = useI18n();
  const { batchDelete } = useBatchActions();

  async function pasteSelected(mode?: "original" | "plain") {
    const id = clipboardStore.selectedId;
    if (id == null || clipboardStore.trashFilter) return;
    const pasteMode =
      mode ??
      (settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original");
    try {
      const injected = await clipboardStore.pasteRecord(id, pasteMode);
      toastPasteOutcome(injected, pasteMode, t, toast);
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
      } catch (e) {
        toast(humanizeInvokeError(e, t), "error");
      }
      return;
    }
    try {
      await clipboardStore.deleteRecord(id);
      toast(t("record.deleted"), "success");
    } catch (e) {
      toast(humanizeInvokeError(e, t), "error");
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

    const action = resolveListHotkey(e, {
      batchMode: clipboardStore.batchMode,
      trashFilter: clipboardStore.trashFilter,
      selectedId: clipboardStore.selectedId,
      selectedCount: clipboardStore.selectedIds.size,
    });
    if (!action) {
      if ((e.ctrlKey || e.metaKey) && (e.key === "d" || e.key === "D")) {
        if (clipboardStore.selectedId == null || clipboardStore.trashFilter) return;
        e.preventDefault();
        void (async () => {
          const id = clipboardStore.selectedId!;
          const record = clipboardStore.selectedRecord;
          if (record && record.is_favorite && !(await confirmUnprotectIfNeeded(record, "favorite"))) return;
          const next = await clipboardStore.toggleFavorite(id);
          if (next == null) toast(t("common.operationFailed"), "error");
        })();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === "t" || e.key === "T")) {
        if (clipboardStore.selectedId == null || clipboardStore.trashFilter) return;
        e.preventDefault();
        void (async () => {
          const id = clipboardStore.selectedId!;
          const record = clipboardStore.selectedRecord;
          if (record && record.is_pinned && !(await confirmUnprotectIfNeeded(record, "pin"))) return;
          const next = await clipboardStore.togglePin(id);
          if (next == null) toast(t("common.operationFailed"), "error");
        })();
      }
      return;
    }

    e.preventDefault();
    switch (action) {
      case "toggle-batch-select":
        if (clipboardStore.selectedId != null) {
          clipboardStore.toggleBatchSelect(clipboardStore.selectedId);
        }
        break;
      case "select-all":
        clipboardStore.selectAllFiltered();
        break;
      case "batch-delete":
        void batchDelete();
        break;
      case "paste":
        void pasteSelected();
        break;
      case "plain-paste":
        void pasteSelected("plain");
        break;
      case "restore":
        if (clipboardStore.selectedId != null) {
          void clipboardStore.restoreRecord(clipboardStore.selectedId);
        }
        break;
      case "delete":
        void deleteSelected();
        break;
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeyDown));
  onUnmounted(() => window.removeEventListener("keydown", onKeyDown));

  return { pasteSelected, deleteSelected };
}
