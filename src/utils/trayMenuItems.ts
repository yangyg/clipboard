import type { AppIconName } from "../components/icons/AppIcon.vue";
import { i18n } from "../locales";

export interface TrayMenuItemDef {
  id: "show" | "pause" | "settings" | "quit";
  label: string;
  icon: AppIconName;
  danger?: boolean;
  separatorBefore?: boolean;
}

export function buildTrayMenuItems(paused: boolean): TrayMenuItemDef[] {
  const t = i18n.global.t;
  return [
    { id: "show", label: t("tray.openPanel"), icon: "panel" },
    {
      id: "pause",
      label: paused ? t("tray.resumeCapture") : t("tray.pauseCapture"),
      icon: paused ? "play" : "pause",
    },
    { id: "settings", label: t("tray.settings"), icon: "settings", separatorBefore: true },
    { id: "quit", label: t("tray.quit"), icon: "close", danger: true, separatorBefore: true },
  ];
}
