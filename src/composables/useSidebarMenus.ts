/**
 * Sidebar quick menu (theme toggle / capture pause / WebDAV sync / help)
 * extracted from SideBar.vue so the SFC script stays under 200 lines.
 */
import { computed, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useFeature } from "./useFeature";
import { useToast } from "./useToast";
import { useI18n } from "vue-i18n";
import type { AppIconName } from "../components/icons/AppIcon.vue";
import type { ContextMenuItem } from "../components/ContextMenu.vue";
import type { Settings, WebDavSyncResult } from "../types";

/** Light-side theme set for the quick-menu dark⇄light toggle (display + action). */
const LIGHT_THEMES = new Set<Settings["theme"]>(["light", "dracula-light", "nord-light", "sunset-light"]);

function isLightTheme(theme: Settings["theme"]): boolean {
  return LIGHT_THEMES.has(theme);
}

export function useSidebarMenus(openSettings: (section?: string) => void) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();
  const { toast } = useToast();
  const { t } = useI18n();
  const syncEnabled = useFeature("sync");

  const webdavSyncing = ref(false);
  const quickMenuAnchorEl = ref<HTMLElement | null>(null);
  const quickMenu = reactive({ visible: false, x: 0, y: 0 });

  const quickMenuItems = computed<ContextMenuItem[]>(() => {
    const items: ContextMenuItem[] = [
      {
        id: "theme-toggle",
        label: t('sidebar.appearance'),
        icon: "palette",
        toggle: {
          value: isLightTheme(settingsStore.settings.theme),
          labels: [t('sidebar.dark'), t('sidebar.light')],
        },
      },
      {
        id: "capture-toggle",
        label: clipboardStore.pauseCapture ? t('sidebar.resumeCapture') : t('sidebar.pauseCapture'),
        icon: (clipboardStore.pauseCapture ? "play" : "pause") as AppIconName,
        separatorBefore: true,
      },
    ];
    if (syncEnabled.value) {
      items.push({
        id: "webdav-sync",
        label: webdavSyncing.value ? t('sidebar.syncing') : t('sidebar.webdavSync'),
        icon: "cloud",
        separatorBefore: true,
      });
    }
    items.push({
      id: "help",
      label: t('sidebar.help'),
      icon: "help",
      separatorBefore: !syncEnabled.value,
    });
    return items;
  });

  function toggleQuickMenu(e: MouseEvent) {
    if (quickMenu.visible) {
      quickMenu.visible = false;
      return;
    }
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    quickMenu.x = rect.left;
    quickMenu.y = rect.top; // ContextMenu clamps into viewport
    quickMenu.visible = true;
  }

  function closeQuickMenu() {
    quickMenu.visible = false;
  }

  function onQuickMenuSelect(id: string) {
    if (id === "theme-toggle") {
      // Flip lightness only: base "light" ⇄ "dark"; colorful presets follow
      // their family side (light ones flip back to dark, dark ones to light).
      const next = isLightTheme(settingsStore.settings.theme) ? "dark" : "light";
      settingsStore.updateSetting("theme", next);
      return;
    }
    if (id === "capture-toggle") {
      clipboardStore.togglePauseCapture();
      return;
    }
    if (id === "webdav-sync") {
      void webdavSync();
      return;
    }
    if (id === "help") {
      quickMenu.visible = false;
      openSettings("help");
      return;
    }
  }

  function isWebDavConfigured(): boolean {
    const s = settingsStore.settings;
    const urlOk = /^https?:\/\/.+/i.test(s.webdav_url.trim());
    return urlOk && s.webdav_username.trim().length > 0 && s.webdav_password.length > 0;
  }

  async function webdavSync() {
    if (webdavSyncing.value) return;
    if (!isWebDavConfigured()) {
      toast(t('sidebar.webdavNotConfigured'), "warning");
      quickMenu.visible = false;
      openSettings("sync");
      return;
    }
    webdavSyncing.value = true;
    try {
      await settingsStore.saveSettings();
      const result = await invoke<WebDavSyncResult>("webdav_sync");
      await settingsStore.loadSettings();
      await clipboardStore.loadRecords();
      await clipboardStore.loadStats();
      toast(result.message || t('sidebar.webdavSyncDone'), "success");
    } catch (e) {
      toast(t('sidebar.webdavSyncFailed', { error: String(e) }), "error");
      quickMenu.visible = false;
      openSettings("sync");
    } finally {
      webdavSyncing.value = false;
      quickMenu.visible = false;
    }
  }

  return {
    webdavSyncing,
    quickMenuAnchorEl,
    quickMenu,
    quickMenuItems,
    toggleQuickMenu,
    closeQuickMenu,
    onQuickMenuSelect,
    isWebDavConfigured,
  };
}
