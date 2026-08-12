/** Source-app display label resolution (list + preview plain-text labels). */
import type { ClipboardRecord, SourceNameOverride } from "../types";

/** vue-i18n translate signature (kept local so this module stays dependency-free). */
export type TranslateFn = (key: string, named?: Record<string, unknown>) => string;

/** Normalize a `source_app` value to a lowercase exe basename for map lookups. */
function normalizeAppKey(sourceApp: string): string {
  return sourceApp.trim().replace(/^.*[/\\]/, "").toLowerCase();
}

/**
 * System / generic Windows apps whose display names should follow the UI
 * language. Values are vue-i18n message keys (see `sourceNames` in locales).
 */
export const SYSTEM_SOURCE_KEYS: Record<string, string> = {
  "notepad.exe": "sourceNames.notepad",
  "mspaint.exe": "sourceNames.paint",
  "explorer.exe": "sourceNames.explorer",
  "cmd.exe": "sourceNames.cmd",
  "powershell.exe": "sourceNames.powershell",
  "pwsh.exe": "sourceNames.powershell",
  "calc.exe": "sourceNames.calculator",
  "winword.exe": "sourceNames.word",
  "excel.exe": "sourceNames.excel",
  "powerpnt.exe": "sourceNames.powerpoint",
  "wsl.exe": "sourceNames.wsl",
  "windowsterminal.exe": "sourceNames.windowsTerminal",
  "winrar.exe": "sourceNames.winrar",
  "7zfm.exe": "sourceNames.sevenZip",
  "taskmgr.exe": "sourceNames.taskmgr",
  "regedit.exe": "sourceNames.regedit",
  "control.exe": "sourceNames.controlPanel",
  "systemsettings.exe": "sourceNames.settings",
};

/**
 * Well-known app brands whose display names are fixed (identical across locales).
 * These also override the FileDescription-based `source_name` so e.g. WeChat
 * always shows 微信 even when the exe's FileDescription is English.
 */
export const BRAND_SOURCE_NAMES: Record<string, string> = {
  "wechat.exe": "微信",
  "weixin.exe": "微信",
  "qq.exe": "QQ",
  "tim.exe": "TIM",
  "wxwork.exe": "企业微信",
  "wework.exe": "企业微信",
  "dingtalk.exe": "钉钉",
  "feishu.exe": "飞书",
  "lark.exe": "飞书",
  "wemeetapp.exe": "腾讯会议",
  "wemeet.exe": "腾讯会议",
  "teams.exe": "Teams",
  "chrome.exe": "Chrome",
  "msedge.exe": "Edge",
  "firefox.exe": "Firefox",
  "iexplore.exe": "IE",
  "360se.exe": "360安全浏览器",
  "qqbrowser.exe": "QQ浏览器",
  "cloudmusic.exe": "网易云音乐",
  "qqmusic.exe": "QQ音乐",
  "potplayer.exe": "PotPlayer",
  "code.exe": "VS Code",
  "devenv.exe": "Visual Studio",
  "rider.exe": "Rider",
  "pycharm64.exe": "PyCharm",
  "idea64.exe": "IntelliJ IDEA",
  "goland64.exe": "GoLand",
  "xmind.exe": "XMind",
  "obsidian.exe": "Obsidian",
  "typora.exe": "Typora",
  "snipaste.exe": "Snipaste",
  "everything.exe": "Everything",
  "listary.exe": "Listary",
  "utools.exe": "uTools",
};

export function sourceShortName(sourceApp: string): string {
  const raw = (sourceApp || "").trim();
  if (!raw) return "系统剪贴板";
  const base = raw.replace(/^.*[/\\]/, "").replace(/\.exe$/i, "");
  return base || raw;
}

/**
 * Resolve the display label for a record's source. Precedence:
 * 1. User-defined override (settings `source_name_overrides`)
 * 2. System app i18n key (locale-aware)
 * 3. Known brand name (fixed)
 * 4. FileDescription stored in `source_name` (Rust capture, long-tail apps)
 * 5. `sourceShortName` fallback (English exe short name)
 */
export function resolveSourceLabel(
  sourceApp: string,
  sourceName: string | undefined,
  t: TranslateFn,
  overrides?: Record<string, string>,
): string {
  const key = normalizeAppKey(sourceApp);
  if (!key) return t("record.systemClipboard");
  if (overrides && overrides[key]) return overrides[key];
  const i18nKey = SYSTEM_SOURCE_KEYS[key];
  if (i18nKey) return t(i18nKey);
  if (key in BRAND_SOURCE_NAMES) return BRAND_SOURCE_NAMES[key];
  const name = (sourceName ?? "").trim();
  if (name) return name;
  return sourceShortName(sourceApp);
}

/** Normalize the settings override list into a lowercase-exe → name map. */
export function buildSourceOverrides(
  overrides: SourceNameOverride[] | undefined | null,
): Record<string, string> {
  const map: Record<string, string> = {};
  for (const o of overrides ?? []) {
    const key = normalizeAppKey(o.exe_name);
    const name = (o.display_name ?? "").trim();
    if (key && name) map[key] = name;
  }
  return map;
}

/**
 * Resolve the device-origin label for a record. Returns "" when the record was
 * captured on this device (or carries no origin) so callers hide the badge.
 * A known device id renders "来自 {name}"; an unknown one falls back to the
 * generic "其他设备" label rather than exposing the raw UUID.
 */
export function resolveDeviceLabel(
  record: Pick<ClipboardRecord, "source_device_id">,
  deviceNames: Record<string, string> | undefined,
  localDeviceId: string | undefined,
  t: TranslateFn,
): string {
  const deviceId = (record.source_device_id ?? "").trim();
  if (!deviceId || deviceId === (localDeviceId ?? "").trim()) return "";
  const name = (deviceNames?.[deviceId] ?? "").trim();
  return name ? t("record.fromDevice", { name }) : t("record.otherDevice");
}

/**
 * Resolve the tooltip for a device-origin badge. Returns "" when the record
 * was captured locally (no badge) so callers hide the tooltip. Uses the same
 * device lookup as `resolveDeviceLabel`; the raw device UUID stays hidden.
 */
export function resolveDeviceTooltip(
  record: Pick<ClipboardRecord, "source_device_id">,
  deviceNames: Record<string, string> | undefined,
  localDeviceId: string | undefined,
  t: TranslateFn,
): string {
  const deviceId = (record.source_device_id ?? "").trim();
  if (!deviceId || deviceId === (localDeviceId ?? "").trim()) return "";
  const name = (deviceNames?.[deviceId] ?? "").trim();
  return name ? t("record.deviceTooltipName", { name }) : t("record.deviceTooltipOther");
}
