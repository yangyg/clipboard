/**
 * App-level Tauri event wiring (clipboard-changed / toggle-panel / focus-loss
 * auto-hide, etc.) extracted from App.vue's onMounted so the SFC script stays
 * under 200 lines. Listener cleanup is owned here (registered on onUnmounted).
 */
import { onMounted, onUnmounted, type Ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Window } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "../stores/clipboard";
import type { ClipboardRecord } from "../types";
import { isPasteFocusLock, setPasteFocusLock } from "./pasteFocusLock";

export interface ClipboardEventsCtx {
  appWindow: Window;
  isWindowMode: () => boolean;
  panelVisible: Ref<boolean>;
  settingsVisible: Ref<boolean>;
  showPanel: () => Promise<void>;
  hidePanel: () => Promise<void>;
  openSettings: (section?: string) => Promise<void>;
}

export function useClipboardEvents(ctx: ClipboardEventsCtx) {
  const clipboardStore = useClipboardStore();
  const unlisteners: Array<() => void> = [];

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

      // Auto-close panel when window loses focus (click outside).
      // When we lose focus the other app is already FG — snapshot it for paste.
      // Skip when custom tray-menu took focus (right-click tray while panel open).
      ctx.appWindow.onFocusChanged(({ payload: focused }) => {
        if (isPasteFocusLock()) return;
        if (!focused && !ctx.isWindowMode()) {
          void (async () => {
            try {
              const tray = await WebviewWindow.getByLabel("tray-menu");
              if (tray && (await tray.isFocused())) return;
            } catch {
              /* ignore */
            }
            void invoke("capture_paste_target").catch((e) =>
              console.debug("[App] capture_paste_target (non-blocking):", e)
            );
            void ctx.hidePanel();
          })();
        }
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
  });

  onUnmounted(() => {
    for (const off of unlisteners) off();
    unlisteners.length = 0;
  });
}
