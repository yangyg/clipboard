/**
 * Record-list interaction handlers (pin animation, soft-delete, context menu,
 * alias dialog, row paste/favorite/restore, list a11y + selection-scroll)
 * extracted from RecordList.vue so the SFC script stays under 200 lines.
 */
import { computed, nextTick, reactive, shallowRef, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useConfirm } from "./useConfirm";
import { useToast } from "./useToast";
import type { ClipboardRecord } from "../types";
import type { ContextMenuItem } from "../components/ContextMenu.vue";
import { toastPasteOutcome } from "../utils/pasteNotify";

export interface RecordActionsCtx {
  listRef: Ref<HTMLElement | null>;
  scrollTop: Ref<number>;
  flatItems: () => ReadonlyArray<{
    type: string;
    id?: number;
    offset: number;
    height: number;
  }>;
  isEmptyOrLoading: () => boolean;
  selectedId: () => number | null;
}

export function useRecordActions(ctx: RecordActionsCtx) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { confirm } = useConfirm();
  const { toast } = useToast();
  const { t } = useI18n();

  /** Optimistic pin icon before list reorders (spec §3.3). */
  const pinOverride = shallowRef(new Map<number, boolean>());
  /** Rows fading out before soft-delete (spec §3.4, restrained). */
  const leavingIds = shallowRef(new Set<number>());

  function isPinned(record: ClipboardRecord): boolean {
    return pinOverride.value.get(record.id) ?? record.is_pinned;
  }

  function sleep(ms: number): Promise<void> {
    if (!settingsStore.settings.enable_animation) return Promise.resolve();
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return Promise.resolve();
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  /** Read a `--transition-*` token ("180ms cubic-bezier(...)") into WAAPI options. */
  function readTokenTransition(name: string): { duration: number; easing: string } {
    const raw = getComputedStyle(document.body).getPropertyValue(name).trim();
    const parts = raw.split(" ");
    const duration = parseFloat(parts[0] ?? "") || 180;
    const easing = parts.slice(1).join(" ") || "ease";
    return { duration, easing };
  }

  /**
   * FLIP-style smooth reflow (spec §3.3/§3.4/§3.6): capture every mounted row's
   * position, run `mutate`, then animate surviving rows from their old offset to
   * the new one. Rows are keyed by data-record-id, so virtualization is fine —
   * only rows currently mounted are measured/played. Uses WAAPI (WebView2
   * supports it) so overlapping flips never fight over inline styles, and it
   * degrades to an instant jump when animations are disabled.
   */
  async function flipAfter<T>(mutate: () => Promise<T> | T): Promise<T> {
    const list = ctx.listRef.value;
    const animate =
      !!list &&
      settingsStore.settings.enable_animation &&
      !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const first = new Map<number, number>();
    if (list) {
      for (const el of list.querySelectorAll<HTMLElement>(".record-item")) {
        const id = Number(el.dataset.recordId);
        if (Number.isFinite(id)) first.set(id, el.getBoundingClientRect().top);
      }
    }
    const result = await mutate();
    if (!list || !animate || first.size === 0) return result;
    await nextTick();
    const { duration, easing } = readTokenTransition("--transition-normal");
    for (const el of list.querySelectorAll<HTMLElement>(".record-item")) {
      const id = Number(el.dataset.recordId);
      const from = first.get(id);
      if (from == null) continue;
      const delta = from - el.getBoundingClientRect().top;
      if (Math.abs(delta) < 1) continue;
      el.animate(
        [{ transform: `translateY(${delta}px)` }, { transform: "none" }],
        { duration, easing, fill: "backwards" },
      );
    }
    return result;
  }

  async function scheduleTogglePin(record: ClipboardRecord) {
    if (leavingIds.value.has(record.id)) return;
    const next = !isPinned(record);
    const pending = new Map(pinOverride.value);
    pending.set(record.id, next);
    pinOverride.value = pending;
    await sleep(150);
    const result = await flipAfter(() => clipboardStore.togglePin(record.id));
    const cleared = new Map(pinOverride.value);
    cleared.delete(record.id);
    pinOverride.value = cleared;
    if (result == null) toast(t('common.operationFailed'), "error");
  }

  const contextMenu = reactive({
    visible: false,
    x: 0,
    y: 0,
    record: null as ClipboardRecord | null,
  });

  const aliasDialog = reactive({
    visible: false,
    recordId: null as number | null,
    initialAlias: "",
  });

  function openAliasDialog(record: ClipboardRecord) {
    aliasDialog.recordId = record.id;
    aliasDialog.initialAlias = record.alias ?? "";
    aliasDialog.visible = true;
  }

  function closeAliasDialog() {
    aliasDialog.visible = false;
    aliasDialog.recordId = null;
    aliasDialog.initialAlias = "";
  }

  const contextMenuItems = computed<ContextMenuItem[]>(() => {
    if (clipboardStore.trashFilter) {
      return [
        { id: "restore", label: t('common.restore'), icon: "restore" },
        { id: "permanentDelete", label: t('record.permanentDelete'), icon: "trash", danger: true, separatorBefore: true },
      ];
    }
    const rec = contextMenu.record;
    return [
      { id: "paste", label: t('common.paste'), icon: "paste", shortcut: "Enter" },
      { id: "pastePlain", label: t('common.pastePlain'), icon: "type", shortcut: "Alt+V" },
      {
        id: "favorite",
        label: rec?.is_favorite ? t('record.unfavorite') : t('record.favorite'),
        icon: "star",
        shortcut: "Ctrl+D",
        separatorBefore: true,
      },
      {
        id: "pin",
        label: rec?.is_pinned ? t('record.unpin') : t('record.pin'),
        icon: "pin",
        shortcut: "Ctrl+T",
      },
      {
        id: "alias",
        label: rec?.alias?.trim() ? t('record.editAlias') : t('record.setAlias'),
        icon: "edit",
      },
      { id: "delete", label: t('common.delete'), icon: "trash", shortcut: "Del / ⌫", danger: true, separatorBefore: true },
    ];
  });

  async function quickPaste(id: number) {
    try {
      const injected = await clipboardStore.pasteRecord(id);
      toastPasteOutcome(injected, "original", t, toast);
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
  }

  async function onRowFavorite(id: number) {
    const next = await clipboardStore.toggleFavorite(id);
    if (next == null) toast(t('common.operationFailed'), "error");
  }

  async function onRowRestore(id: number) {
    try {
      await clipboardStore.restoreRecord(id);
    } catch {
      toast(t('common.operationFailed'), "error");
    }
  }

  async function quickDelete(record: ClipboardRecord) {
    if (clipboardStore.trashFilter) {
      const ok = await confirm({
        title: t('record.permanentDelete'),
        message: t('record.permanentDeleteMsg'),
        confirmText: t('record.permanentDelete'),
        danger: true,
      });
      if (ok) {
        try {
          await clipboardStore.permanentlyDeleteRecord(record.id);
          toast(t('record.deletedPermanently'), "success");
        } catch {
          toast(t('common.operationFailed'), "error");
        }
      }
      return;
    }
    if (leavingIds.value.has(record.id)) return;
    const nextLeave = new Set(leavingIds.value);
    nextLeave.add(record.id);
    leavingIds.value = nextLeave;
    await sleep(160);
    try {
      await flipAfter(() => clipboardStore.deleteRecord(record.id));
      toast(t('record.deleted'), "success");
    } catch {
      toast(t('common.operationFailed'), "error");
    } finally {
      const cleared = new Set(leavingIds.value);
      cleared.delete(record.id);
      leavingIds.value = cleared;
    }
  }

  function onItemClick(id: number) {
    if (clipboardStore.batchMode) {
      clipboardStore.toggleBatchSelect(id);
      return;
    }
    clipboardStore.selectRecord(id);
  }

  /** Enter activates paste (or restore in trash). Double-click removed — easy to misfire. */
  async function onItemActivate(id: number) {
    if (clipboardStore.batchMode) {
      clipboardStore.toggleBatchSelect(id);
      return;
    }
    if (clipboardStore.trashFilter) {
      try {
        await clipboardStore.restoreRecord(id);
      } catch {
        toast(t('common.operationFailed'), "error");
      }
      return;
    }
    try {
      const injected = await clipboardStore.pasteRecord(id);
      toastPasteOutcome(injected, "original", t, toast);
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
  }

  function showContextMenu(e: MouseEvent, record: ClipboardRecord) {
    contextMenu.visible = true;
    contextMenu.x = e.clientX;
    contextMenu.y = e.clientY;
    contextMenu.record = record;
  }

  async function onContextSelect(id: string) {
    const record = contextMenu.record;
    contextMenu.visible = false;
    if (!record) return;

    if (id === "paste") {
      try {
        const injected = await clipboardStore.pasteRecord(record.id);
        toastPasteOutcome(injected, "original", t, toast);
      } catch {
        toast(t('record.pasteFailed'), "error");
      }
      return;
    }
    if (id === "pastePlain") {
      try {
        const injected = await clipboardStore.pasteRecord(record.id, "plain");
        toastPasteOutcome(injected, "plain", t, toast);
      } catch {
        toast(t('record.pasteFailed'), "error");
      }
      return;
    }
    if (id === "favorite") {
      const next = await clipboardStore.toggleFavorite(record.id);
      if (next == null) toast(t('common.operationFailed'), "error");
      return;
    }
    if (id === "pin") {
      await scheduleTogglePin(record);
      return;
    }
    if (id === "alias") {
      openAliasDialog(record);
      return;
    }
    if (id === "restore") {
      try {
        await clipboardStore.restoreRecord(record.id);
      } catch {
        toast(t('common.operationFailed'), "error");
      }
      return;
    }
    if (id === "delete") {
      await quickDelete(record);
      return;
    }
    if (id === "permanentDelete") {
      const ok = await confirm({
        title: t('record.permanentDelete'),
        message: t('record.permanentDeleteMsg'),
        confirmText: t('record.permanentDelete'),
        danger: true,
      });
      if (ok) {
        try {
          await clipboardStore.permanentlyDeleteRecord(record.id);
          toast(t('record.deletedPermanently'), "success");
        } catch {
          toast(t('common.operationFailed'), "error");
        }
      }
    }
  }

  function closeContextMenu() {
    contextMenu.visible = false;
  }

  // ── List accessibility + selection scroll ──

  /** Back-to-top: reveal once scrolled past a fixed threshold (~1 viewport). */
  const BACK_TO_TOP_THRESHOLD = 400;
  const showBackToTop = computed(
    () => !ctx.isEmptyOrLoading() && ctx.scrollTop.value > BACK_TO_TOP_THRESHOLD
  );

  function scrollToTop() {
    const el = ctx.listRef.value;
    if (!el) return;
    const reduceMotion =
      !settingsStore.settings.enable_animation ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollTo({ top: 0, behavior: reduceMotion ? "auto" : "smooth" });
  }

  const activeDescendantId = computed(() =>
    ctx.selectedId() != null ? `record-option-${ctx.selectedId()}` : undefined
  );

  const firstRecordId = computed(() => {
    for (const it of ctx.flatItems()) {
      if (it.type === "record" && it.id != null) return it.id;
    }
    return null;
  });

  function isOptionTabbable(id: number): boolean {
    if (ctx.selectedId() === id) return true;
    if (ctx.selectedId() == null && firstRecordId.value === id) return true;
    return false;
  }

  // Keep the selected row visible as selection moves (jump by virtual offset
  // when the row is not mounted in the window).
  watch(
    () => ctx.selectedId(),
    async (id) => {
      if (id == null) return;
      await nextTick();
      const list = ctx.listRef.value;
      if (!list) return;
      const mounted = list.querySelector(`[data-record-id="${id}"]`) as HTMLElement | null;
      if (mounted) {
        mounted.scrollIntoView({ block: "nearest" });
        return;
      }
      // Selected row may be outside the virtual window — jump by layout offset.
      const target = ctx.flatItems().find((it) => it.id === id);
      if (!target) return;
      const viewH = list.clientHeight;
      const top = target.offset;
      const bottom = top + target.height;
      if (top < list.scrollTop) list.scrollTop = top;
      else if (bottom > list.scrollTop + viewH) list.scrollTop = bottom - viewH;
      ctx.scrollTop.value = list.scrollTop;
    }
  );

  return {
    pinOverride,
    leavingIds,
    isPinned,
    scheduleTogglePin,
    contextMenu,
    contextMenuItems,
    showContextMenu,
    closeContextMenu,
    onContextSelect,
    aliasDialog,
    openAliasDialog,
    closeAliasDialog,
    quickPaste,
    onRowFavorite,
    onRowRestore,
    quickDelete,
    onItemClick,
    onItemActivate,
    showBackToTop,
    scrollToTop,
    activeDescendantId,
    isOptionTabbable,
  };
}
