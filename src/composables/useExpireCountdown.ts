/**
 * Live sensitive-expiry countdown for the preview pane, extracted from
 * PreviewPane.vue so the SFC script stays under 200 lines.
 */
import { computed, onUnmounted, ref, watch, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord } from "../types";
import { expireBannerKind } from "../utils/sensitiveExpiry";

/** Live countdown — always include seconds so the UI visibly ticks. */
function formatRemainMs(ms: number): string {
  const totalSec = Math.ceil(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  if (m > 0) return `${m}:${String(s).padStart(2, "0")}`;
  return `${s}s`;
}

export function useExpireCountdown(record: ComputedRef<ClipboardRecord | null>) {
  const { t } = useI18n();
  const expireNow = ref(Date.now());
  let expireTimer: ReturnType<typeof setInterval> | null = null;

  function clearExpireTimer() {
    if (expireTimer) {
      clearInterval(expireTimer);
      expireTimer = null;
    }
  }

  function startOrStopTimer() {
    clearExpireTimer();
    const iso = record.value?.auto_expire_at ?? null;
    if (!iso) return;
    expireNow.value = Date.now();
    const at = new Date(iso).getTime();
    if (Number.isNaN(at) || at <= expireNow.value) return;
    expireTimer = setInterval(() => {
      expireNow.value = Date.now();
      if (expireNow.value >= at) clearExpireTimer();
    }, 1000);
  }

  watch(
    () => [
      record.value?.auto_expire_at ?? null,
      record.value?.is_pinned ?? false,
      record.value?.is_favorite ?? false,
    ],
    startOrStopTimer,
    { immediate: true },
  );

  onUnmounted(() => {
    clearExpireTimer();
  });

  const expireText = computed(() => {
    const rec = record.value;
    if (!rec) return "";
    const kind = expireBannerKind(rec, expireNow.value);
    if (!kind) return "";
    if (kind === "expired") return t("preview.expired");
    if (kind === "protected-expired") return t("preview.expiredProtected");
    const iso = rec.auto_expire_at;
    if (!iso) return "";
    const ms = new Date(iso).getTime() - expireNow.value;
    const time = formatRemainMs(Math.max(ms, 0));
    if (kind === "protected-countdown") {
      return t("preview.autoExpireProtected", { time });
    }
    return t("preview.autoExpire", { time });
  });

  const expireTitle = computed(() => {
    const rec = record.value;
    if (!rec) return "";
    return expireBannerKind(rec, expireNow.value) === "protected-expired"
      ? t("preview.expiredProtectedHint")
      : "";
  });

  return { expireText, expireTitle };
}
