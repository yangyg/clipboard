/**
 * Live sensitive-expiry countdown for the preview pane, extracted from
 * PreviewPane.vue so the SFC script stays under 200 lines.
 */
import { onUnmounted, ref, watch, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord } from "../types";

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

  watch(
    () => record.value?.auto_expire_at ?? null,
    (iso) => {
      clearExpireTimer();
      if (!iso) return;
      expireNow.value = Date.now();
      expireTimer = setInterval(() => {
        expireNow.value = Date.now();
      }, 1000);
    },
    { immediate: true }
  );

  onUnmounted(() => {
    clearExpireTimer();
  });

  /** Live countdown — always include seconds so the UI visibly ticks. */
  function formatExpireTime(iso: string): string {
    const ms = new Date(iso).getTime() - expireNow.value;
    if (ms <= 0) return t('preview.expired');
    const totalSec = Math.ceil(ms / 1000);
    const m = Math.floor(totalSec / 60);
    const s = totalSec % 60;
    if (m > 0) return `${m}:${String(s).padStart(2, "0")}`;
    return `${s}s`;
  }

  return { formatExpireTime };
}
