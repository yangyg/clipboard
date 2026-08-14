/**
 * Shared confirmation guard for un-pin / un-favorite. Warns before removing
 * the last protection on an already-expired sensitive record, which the next
 * `cleanup_expired` sweep will hard-delete (not trash). No-op for every other
 * toggle so routine pin/favorite stays friction-free.
 */
import { useI18n } from "vue-i18n";
import type { ClipboardRecord } from "../types";
import { needsExpiryConfirm } from "../utils/sensitiveExpiry";
import { useConfirm } from "./useConfirm";

export function useExpiryGuard() {
  const { confirm } = useConfirm();
  const { t } = useI18n();

  /**
   * Resolves `true` when it is safe to proceed (no expiry risk, or the user
   * confirmed the permanent deletion). Callers must only invoke this when the
   * action actually removes `kind` (un-pin / un-favorite).
   */
  async function confirmUnprotectIfNeeded(
    record: ClipboardRecord,
    kind: "pin" | "favorite",
  ): Promise<boolean> {
    if (!needsExpiryConfirm(record, kind)) return true;
    return confirm({
      title: t("record.expiryConfirmTitle"),
      message: t("record.expiryConfirmMsg"),
      confirmText: kind === "pin" ? t("record.unpin") : t("record.unfavorite"),
      danger: true,
    });
  }

  return { confirmUnprotectIfNeeded };
}
