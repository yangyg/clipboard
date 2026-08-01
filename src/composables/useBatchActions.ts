import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useClipboardStore } from "../stores/clipboard";
import { useToast } from "./useToast";

/** Shared batch bar actions for FloatingPanel and WindowApp. */
export function useBatchActions() {
  const clipboardStore = useClipboardStore();
  const { toast } = useToast();
  const { t } = useI18n();

  function toggleBatchMode() {
    clipboardStore.toggleBatchMode();
  }

  async function batchCopy() {
    const idSet = clipboardStore.selectedIds;
    if (!idSet.size) {
      toast(t("batch.selectFirst"), "warning");
      return;
    }
    const selected = clipboardStore.records.filter((r) => idSet.has(r.id));
    if (!selected.length) return;

    const images = selected.filter((r) => r.content_type === "image");
    if (images.length === selected.length) {
      if (images.length === 1) {
        try {
          await clipboardStore.pasteRecord(images[0].id, "original");
          toast(t("batch.pastedImage"), "success");
        } catch {
          toast(t("record.pasteFailed"), "error");
        }
        return;
      }
      toast(t("batch.multiImageUnsupported"), "warning");
      return;
    }
    if (images.length > 0) {
      toast(t("batch.skippedImages"), "warning");
    }
    const textIds = selected
      .filter((r) => r.content_type !== "image")
      .map((r) => r.id);
    // List rows truncate content to 400 chars — fetch full bodies for copy.
    const fullTexts: string[] = [];
    try {
      const results = await Promise.all(
        textIds.map((id) => invoke<{ content: string } | null>("get_record", { id }))
      );
      for (const full of results) {
        if (full?.content) fullTexts.push(full.content);
      }
    } catch {
      toast(t("batch.readFullFailed"), "error");
      return;
    }
    const text = fullTexts.join("\n\n");
    if (!text.trim()) {
      toast(t("batch.noText"), "warning");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      toast(t("batch.copied", { n: fullTexts.length }), "success");
    } catch {
      toast(t("batch.copyFailed"), "error");
    }
  }

  async function batchFavorite() {
    const ids = Array.from(clipboardStore.selectedIds);
    if (!ids.length) {
      toast(t("batch.selectFirst"), "warning");
      return;
    }
    try {
      await clipboardStore.batchFavorite(ids);
    } catch {
      toast(t("common.operationFailed"), "error");
    }
  }

  async function batchDelete() {
    const ids = Array.from(clipboardStore.selectedIds);
    if (!ids.length) {
      toast(t("batch.selectFirst"), "warning");
      return;
    }
    try {
      await clipboardStore.deleteBatch(ids);
      toast(t("record.deleted"), "success");
    } catch {
      toast(t("common.operationFailed"), "error");
    }
  }

  return { toggleBatchMode, batchCopy, batchFavorite, batchDelete };
}
