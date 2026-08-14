/**
 * Expiry-sweep + stats-debounce store actions extracted from clipboard.ts to
 * reduce file size. The store public API is unchanged — methods are spread
 * back into the store return.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type { ClipboardRecord, StatsData } from "../types";
import { featureEnabled } from "../composables/useFeature";
import { detailRemove } from "./clipboardList";

export interface ExpirySchedulerCtx {
  records: Ref<ClipboardRecord[]>;
  selectedId: Ref<number | null>;
  selectedIds: Ref<Set<number>>;
  recordDetails: Ref<Map<number, ClipboardRecord>>;
  stats: Ref<StatsData | null>;
}

/**
 * True for rows the backend `cleanup_expired` also skips: favorite/pinned
 * (user-kept sensitive records) and trashed (owned by the trash-retention
 * window). The frontend sweep must mirror this so a protected record that has
 * passed its auto-expiry neither disappears from the list nor re-triggers the
 * sweep loop.
 */
function isProtectedFromExpiry(r: ClipboardRecord): boolean {
  return r.is_favorite || r.is_pinned || r.is_trashed;
}

export function createExpiryScheduler(ctx: ExpirySchedulerCtx) {
  let expireSweepTimer: ReturnType<typeof setTimeout> | null = null;
  let expireSweepRunning = false;
  let statsDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let statsMaxWaitTimer: ReturnType<typeof setTimeout> | null = null;

  /** Remove expired sensitive records from DB + local list; reschedule next sweep. */
  function removeExpiredFromList(ids: number[]) {
    if (ids.length === 0) return;
    const idSet = new Set(ids);
    ctx.records.value = ctx.records.value.filter((r) => !idSet.has(r.id));
    if (ctx.selectedId.value !== null && idSet.has(ctx.selectedId.value)) {
      ctx.selectedId.value = null;
    }
    if (ctx.selectedIds.value.size > 0) {
      const next = new Set([...ctx.selectedIds.value].filter((id) => !idSet.has(id)));
      ctx.selectedIds.value = next;
    }
    detailRemove(ctx.recordDetails, [...idSet]);
  }

  async function purgeExpiredRecords() {
    if (expireSweepRunning) return;
    expireSweepRunning = true;
    try {
      const ids = await invoke<number[]>("cleanup_expired");
      removeExpiredFromList(ids);
      // Also drop any locally past-due rows (clock skew / missed event).
      // Skip favorite/pinned/trashed rows to mirror the backend
      // `cleanup_expired` WHERE clause — a protected sensitive record must not
      // vanish from the list just because its auto-expiry passed.
      const now = Date.now();
      const stale = ctx.records.value
        .filter(
          (r) =>
            r.auto_expire_at &&
            new Date(r.auto_expire_at).getTime() <= now &&
            !isProtectedFromExpiry(r),
        )
        .map((r) => r.id);
      if (stale.length > 0) {
        removeExpiredFromList(stale);
      }
      if (ids.length > 0 || stale.length > 0) {
        scheduleLoadStats();
      }
    } catch (e) {
      console.error("Purge expired failed:", e);
    } finally {
      expireSweepRunning = false;
      scheduleExpireSweep();
    }
  }

  function scheduleExpireSweep() {
    if (expireSweepTimer) {
      clearTimeout(expireSweepTimer);
      expireSweepTimer = null;
    }
    const now = Date.now();
    let nextAt = Infinity;
    for (const r of ctx.records.value) {
      // Protected rows (favorite/pinned/trashed) are never swept by the
      // backend, so they must not drive the reschedule either — otherwise a
      // protected past-due row would re-trigger the sweep in a tight loop.
      if (!r.auto_expire_at || isProtectedFromExpiry(r)) continue;
      // M-1: Only parse the nearest timestamp (one Date parse vs N).
      const t = new Date(r.auto_expire_at).getTime();
      if (Number.isNaN(t)) continue;
      if (t <= now) {
        void purgeExpiredRecords();
        return;
      }
      if (t < nextAt) nextAt = t;
    }
    if (nextAt < Infinity) {
      const delay = Math.max(50, nextAt - Date.now() + 30);
      expireSweepTimer = setTimeout(() => {
        expireSweepTimer = null;
        void purgeExpiredRecords();
      }, delay);
    }
  }

  /** Debounce 800ms while idle; max-wait 5s so continuous copy still refreshes stats. */
  function scheduleLoadStats() {
    if (!featureEnabled("stats")) return;
    if (statsDebounceTimer) clearTimeout(statsDebounceTimer);
    statsDebounceTimer = setTimeout(() => {
      statsDebounceTimer = null;
      if (statsMaxWaitTimer) {
        clearTimeout(statsMaxWaitTimer);
        statsMaxWaitTimer = null;
      }
      void loadStats();
    }, 800);

    if (!statsMaxWaitTimer) {
      statsMaxWaitTimer = setTimeout(() => {
        statsMaxWaitTimer = null;
        if (statsDebounceTimer) {
          clearTimeout(statsDebounceTimer);
          statsDebounceTimer = null;
        }
        void loadStats();
      }, 5000);
    }
  }

  async function loadStats() {
    if (!featureEnabled("stats")) return;
    try {
      ctx.stats.value = await invoke<StatsData>("get_stats");
    } catch (e) {
      console.error("Load stats failed:", e);
    }
  }

  return {
    removeExpiredFromList,
    purgeExpiredRecords,
    scheduleExpireSweep,
    scheduleLoadStats,
    loadStats,
  };
}
