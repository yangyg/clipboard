import { useClipboardStore } from "../stores/clipboard";
import { useToast } from "./useToast";

/** Shared batch bar actions for FloatingPanel and WindowApp. */
export function useBatchActions() {
  const clipboardStore = useClipboardStore();
  const { toast } = useToast();

  function toggleBatchMode() {
    clipboardStore.toggleBatchMode();
  }

  async function batchCopy() {
    const ids = Array.from(clipboardStore.selectedIds);
    if (!ids.length) {
      toast("请先选择条目", "warning");
      return;
    }
    const selected = clipboardStore.records.filter((r) => ids.includes(r.id));
    if (!selected.length) return;

    const images = selected.filter((r) => r.content_type === "image");
    if (images.length === selected.length) {
      if (images.length === 1) {
        try {
          await clipboardStore.pasteRecord(images[0].id, "original");
          toast("已粘贴图片", "success");
        } catch {
          toast("粘贴失败", "error");
        }
        return;
      }
      toast("批量复制暂不支持多张图片，请单条粘贴", "warning");
      return;
    }
    if (images.length > 0) {
      toast("已跳过图片，仅复制文本内容", "warning");
    }
    const text = selected
      .filter((r) => r.content_type !== "image")
      .map((r) => r.content)
      .join("\n\n");
    if (!text.trim()) {
      toast("没有可复制的文本", "warning");
      return;
    }
    await navigator.clipboard.writeText(text);
    toast(`已复制 ${selected.length - images.length} 项到剪贴板`, "success");
  }

  async function batchFavorite() {
    const ids = Array.from(clipboardStore.selectedIds);
    if (!ids.length) {
      toast("请先选择条目", "warning");
      return;
    }
    await clipboardStore.batchFavorite(ids);
  }

  async function batchDelete() {
    const ids = Array.from(clipboardStore.selectedIds);
    if (!ids.length) {
      toast("请先选择条目", "warning");
      return;
    }
    await clipboardStore.deleteBatch(ids);
    toast("已移到回收站", "success");
  }

  return { toggleBatchMode, batchCopy, batchFavorite, batchDelete };
}
