/** Stable colors for source letter avatars (same palette as the old list source-dot). */
export const SOURCE_AVATAR_PALETTE = [
  "#0078d4",
  "#34d399",
  "#fbbf24",
  "#f87171",
  "#38bdf8",
  "#a78bfa",
  "#fb923c",
  "#94a3b8",
] as const;

const EMPTY_COLOR = "var(--text-tertiary)";

export function sourceShortName(sourceApp: string): string {
  const raw = (sourceApp || "").trim();
  if (!raw) return "系统剪贴板";
  const base = raw.replace(/^.*[/\\]/, "").replace(/\.exe$/i, "");
  return base || raw;
}

export function sourceInitial(shortName: string, sourceApp: string): string {
  if (!(sourceApp || "").trim()) return "剪";
  const latin = shortName.match(/[A-Za-z0-9]/);
  if (latin) return latin[0].toUpperCase();
  const first = [...shortName][0];
  return first || "剪";
}

export function sourceAvatarColor(sourceApp: string): string {
  const s = (sourceApp || "").trim();
  if (!s) return EMPTY_COLOR;
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return SOURCE_AVATAR_PALETTE[h % SOURCE_AVATAR_PALETTE.length];
}

export function resolveSourceBadge(sourceApp: string): {
  label: string;
  initial: string;
  color: string;
} {
  const label = sourceShortName(sourceApp);
  return {
    label,
    initial: sourceInitial(label, sourceApp),
    color: sourceAvatarColor(sourceApp),
  };
}
