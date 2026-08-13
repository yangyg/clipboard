/**
 * Shared date/time + byte formatting. These used to be re-implemented with
 * slightly different options in preview, settings-data and settings-sync;
 * keeping them here means the output is identical everywhere.
 */

/** Absolute timestamp with a stable field set (optional seconds). */
export function formatDateTime(iso: string, withSeconds = false): string {
  return new Date(iso).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    ...(withSeconds ? { second: "2-digit" as const } : {}),
  });
}

/** Compact human-readable byte size (B / KB / MB). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
