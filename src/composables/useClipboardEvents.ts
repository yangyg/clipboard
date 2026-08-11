/**
 * App-level Tauri event wiring (clipboard-changed / toggle-panel / focus-loss
 * auto-hide, etc.) extracted from App.vue's onMounted so the SFC script stays
 * under 200 lines. Listener cleanup is owned here (registered on onUnmounted).
 */
import { onMounted, onUnmounted, type Ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Window } from "@tauri-apps/api/window";
import { useClipboardStore } from "../stores/clipboard";
import type { ClipboardRecord } from "../types";
import { isPasteFocusLock, setPasteFocusLock } from "./pasteFocusLock";
import { useToast } from "./useToast";
import { i18n } from "../locales";

const { toast } = useToast();

export interface ClipboardEventsCtx {
  appWindow: Window;
  panelVisible: Ref<boolean>;
  settingsVisible: Ref<boolean>;
  showPanel: () => Promise<void>;
  hidePanel: () => void;
  openSettings: (section?: string) => Promise<void>;
}

export function useClipboardEvents(ctx: ClipboardEventsCtx) {
  const clipboardStore = useClipboardStore();
  const unlisteners: Array<() => void> = [];
  let historyImportApplied = false;

  // Exactly-once handling shared by the live event and the boot catch-up:
  // whichever path delivers the count first applies it (reload + stats + toast).
  const applyHistoryImport = (inserted: number) => {
    if (historyImportApplied || inserted <= 0) return;
    historyImportApplied = true;
    void clipboardStore.reloadList();
    clipboardStore.scheduleLoadStats();
    toast(i18n.global.t("settings.history.importDone", { count: inserted }), "success");
  };

  onMounted(async () => {
    // Register all event listeners concurrently. A single rejected listen
    // (e.g. a plugin event channel missing in an old runtime) must not stop
    // the remaining listeners from being wired up.
    const registrations = [
      // Listen for new clipboard records from Rust backend
      listen<ClipboardRecord>("clipboard-changed", (event) => {
        if (!clipboardStore.pauseCapture) {
          clipboardStore.onNewRecord(event.payload);
        }
      }),

// Sensitive auto-expire deleted in Rust (periodic cleanup thread) → sync list
      listen<number[]>("records-expired", (event) => {
        clipboardStore.removeExpiredFromList(event.payload ?? []);
        clipboardStore.scheduleLoadStats();
      }),

      // Startup import of the OS clipboard history finished → refresh once
      listen<{ inserted: number }>("clipboard-history-imported", (event) => {
        applyHistoryImport(event.payload?.inserted ?? 0);
      }),

      // Listen for toggle-panel from Rust (Rust shows/hides window, we sync panelVisible)
      listen<boolean>("toggle-panel", (event) => {
        if (isPasteFocusLock() && event.payload) {
          // Mid-paste / keep-open: sync flag only — never setFocus (would steal from target).
          ctx.panelVisible.value = true;
          return;
        }
        if (event.payload) {
          if (!ctx.panelVisible.value || ctx.settingsVisible.value) {
            void ctx.showPanel();
          } else {
            // Already visible — still show/focus window without forcing reload
            void ctx.appWindow.show().then(() => ctx.appWindow.setFocus());
          }
        } else {
          if (ctx.panelVisible.value) {
            void ctx.hidePanel();
          }
        }
      }),

listen<boolean>("paste-focus-lock", (event) => {
        setPasteFocusLock(!!event.payload);
      }),

      // Listen for open-settings from Rust tray menu
      listen("open-settings", () => {
        void ctx.openSettings();
      }),

      // Tray pause/resume syncs Rust → frontend
      listen<boolean>("capture-paused", (event) => {
        clipboardStore.setPauseCapture(event.payload);
      }),
    ];
    const settled = await Promise.allSettled(registrations);
for (const result of settled) {
      if (result.status === "fulfilled") {
        unlisteners.push(result.value);
      } else {
        console.error("[App] failed to register Tauri event listener:", result.reason);
      }
    }

    // Catch-up for the startup import: it is triggered by the first `Focused`
    // event, which can complete before these listeners were registered. Read
    // the pending result once; `applyHistoryImport` dedups against the live event.
    invoke<number | null>("get_pending_history_import")
      .then((inserted) => applyHistoryImport(inserted ?? 0))
      .catch(() => {
        /* non-critical catch-up */
      });
  });

  onUnmounted(() => {
    for (const off of unlisteners) off();
    unlisteners.length = 0;
  });
}
