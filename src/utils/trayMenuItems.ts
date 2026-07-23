import type { AppIconName } from "../components/icons/AppIcon.vue";

export interface TrayMenuItemDef {
  id: "show" | "pause" | "settings" | "quit";
  label: string;
  icon: AppIconName;
  danger?: boolean;
  separatorBefore?: boolean;
}

export function buildTrayMenuItems(paused: boolean): TrayMenuItemDef[] {
  return [
    { id: "show", label: "打开面板", icon: "panel" },
    {
      id: "pause",
      label: paused ? "恢复捕获" : "暂停捕获",
      icon: paused ? "play" : "pause",
    },
    { id: "settings", label: "设置", icon: "settings", separatorBefore: true },
    { id: "quit", label: "退出", icon: "close", danger: true, separatorBefore: true },
  ];
}
