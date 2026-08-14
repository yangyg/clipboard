/**
 * Preview action handlers (paste / favorite / pin / delete / tag / open
 * external) extracted from PreviewPane.vue so the SFC script stays under 200
 * lines. Also owns the optimistic pin state and the tag-assign dialog refs.
 */
import { computed, ref, watch, type ComputedRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useConfirm } from "./useConfirm";
import { useToast } from "./useToast";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord, Tag } from "../types";
import { toastPasteOutcome } from "../utils/pasteNotify";

export interface PreviewActionsCtx {
  record: ComputedRef<ClipboardRecord | null>;
  openableLinkUrl: ComputedRef<string | null>;
  tagsByName: ComputedRef<Map<string, Tag>>;
}

export function usePreviewActions(ctx: PreviewActionsCtx) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { confirm } = useConfirm();
  const { toast } = useToast();
  const { t } = useI18n();

  const tagDialogVisible = ref(false);
  const tagDialogMode = ref<"assign" | "create">("assign");

  /** Optimistic pin label/icon before list reorders. */
  const pinOverride = ref<boolean | null>(null);
  watch(
    () => ctx.record.value?.id,
    () => {
      pinOverride.value = null;
    },
  );

  const pinnedDisplay = computed(() => {
    if (pinOverride.value !== null) return pinOverride.value;
    return !!ctx.record.value?.is_pinned;
  });

  async function openImageExternally() {
    const id = ctx.record.value?.id;
    if (id == null) return;
    try {
      await invoke("open_record_media", { id });
    } catch (e) {
      console.error("Open image failed:", e);
      const msg = typeof e === "string" ? e : t('preview.openImageFailed');
      toast(msg, "error");
    }
  }

  async function openLinkExternally() {
    const url = ctx.openableLinkUrl.value;
    if (!url) return;
    try {
      await invoke("open_url", { url });
    } catch (e) {
      console.error("Open link failed:", e);
      const msg = typeof e === "string" ? e : t("preview.openLinkFailed");
      toast(msg, "error");
    }
  }

  function openTagAssign() {
    tagDialogMode.value = "assign";
    tagDialogVisible.value = true;
  }

  async function removeTag(tagName: string) {
    if (!ctx.record.value) return;
    const tag = ctx.tagsByName.value.get(tagName);
    if (tag) {
      await clipboardStore.removeTagFromRecord(ctx.record.value.id, tag.id, tagName);
    }
  }

  function onTagCreated() {
    tagDialogMode.value = "assign";
  }

  async function paste() {
    if (!ctx.record.value) return;
    const mode = settingsStore.settings.default_paste_mode === "plain" ? "plain" : "original";
    try {
      const injected = await clipboardStore.pasteRecord(ctx.record.value.id, mode);
      toastPasteOutcome(injected, mode, t, toast);
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
  }

  async function pastePlain() {
    if (!ctx.record.value) return;
    try {
      const injected = await clipboardStore.pasteRecord(ctx.record.value.id, "plain");
      toastPasteOutcome(injected, "plain", t, toast);
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
  }

  async function favorite() {
    if (!ctx.record.value) return;
    const next = await clipboardStore.toggleFavorite(ctx.record.value.id);
    if (next == null) toast(t('common.operationFailed'), "error");
  }

  async function pin() {
    if (!ctx.record.value) return;
    const id = ctx.record.value.id;
    pinOverride.value = !pinnedDisplay.value;
    if (
      settingsStore.settings.enable_animation &&
      !window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      await new Promise((r) => setTimeout(r, 150));
    }
    if (clipboardStore.selectedId !== id) {
      pinOverride.value = null;
      return;
    }
    const next = await clipboardStore.togglePin(id);
    pinOverride.value = null;
    if (next == null) toast(t('common.operationFailed'), "error");
  }

  async function del() {
    if (!ctx.record.value) return;
    try {
      await clipboardStore.deleteRecord(ctx.record.value.id);
      toast(t('record.deleted'), "success");
    } catch {
      toast(t('common.operationFailed'), "error");
    }
  }

  async function restore() {
    if (!ctx.record.value) return;
    try {
      await clipboardStore.restoreRecord(ctx.record.value.id);
    } catch {
      toast(t('common.operationFailed'), "error");
    }
  }

  async function permanentDel() {
    if (!ctx.record.value) return;
    const ok = await confirm({
      title: t('record.permanentDelete'),
      message: t('record.permanentDeleteMsg'),
      confirmText: t('record.permanentDelete'),
      danger: true,
    });
    if (ok) {
      try {
        await clipboardStore.permanentlyDeleteRecord(ctx.record.value.id);
        toast(t('record.deletedPermanently'), "success");
      } catch {
        toast(t('common.operationFailed'), "error");
      }
    }
  }

  return {
    pinnedDisplay,
    tagDialogVisible,
    tagDialogMode,
    openImageExternally,
    openLinkExternally,
    openTagAssign,
    removeTag,
    onTagCreated,
    paste,
    pastePlain,
    favorite,
    pin,
    del,
    restore,
    permanentDel,
  };
}
