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
      // Also drop any locally past-due rows (clock skew / missed event)
      const now = Date.now();
      const stale = ctx.records.value
        .filter((r) => r.auto_expire_at && new Date(r.auto_expire_at).getTime() <= now)
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
      if (!r.auto_expire_at) continue;
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
