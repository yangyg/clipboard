/**
 * Pure predicates for the sensitive auto-expiry lifecycle, shared by the
 * frontend sweep (`stores/clipboardExpiry.ts`) and the un-protect confirmation
 * guards. Keep this free of Vue/Tauri state so it stays unit-testable in
 * isolation — the semantics mirror the backend `cleanup_expired` WHERE clause
 * (`is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0`).
 */
import type { ClipboardRecord } from "../types";

/** True when pin or favorite shields the row from `cleanup_expired`. */
export function isExpiryProtected(record: ClipboardRecord): boolean {
  return record.is_pinned || record.is_favorite;
}

/** True when the record's auto-expiry timestamp is already in the past. */
export function isExpired(record: ClipboardRecord, now = Date.now()): boolean {
  if (!record.auto_expire_at) return false;
  const at = new Date(record.auto_expire_at).getTime();
  return !Number.isNaN(at) && at <= now;
}

/**
 * Preview warning-bar copy state. `null` when there is no expiry timestamp
 * (or it is unparsable) — the countdown span stays hidden.
 */
export type ExpireBannerKind =
  | "countdown"
  | "expired"
  | "protected-countdown"
  | "protected-expired";

export function expireBannerKind(
  record: ClipboardRecord,
  now = Date.now(),
): ExpireBannerKind | null {
  if (!record.auto_expire_at) return null;
  const at = new Date(record.auto_expire_at).getTime();
  if (Number.isNaN(at)) return null;
  const expired = at <= now;
  if (isExpiryProtected(record)) {
    return expired ? "protected-expired" : "protected-countdown";
  }
  return expired ? "expired" : "countdown";
}

/**
 * True when removing `kind` protection (pin or favorite) would drop the last
 * shield on an already-expired sensitive record — the backend will then
 * hard-delete it (not trash) on the next cleanup. Used to gate the un-pin /
 * un-favorite confirmation so routine toggles stay friction-free.
 */
export function needsExpiryConfirm(
  record: ClipboardRecord,
  kind: "pin" | "favorite",
): boolean {
  if (!record.is_sensitive || !isExpired(record)) return false;
  // Removing pin leaves favorite as the only shield; removing favorite leaves pin.
  return kind === "pin" ? !record.is_favorite : !record.is_pinned;
}

/**
 * Slider label parts for `sensitive_auto_expire_seconds`. Matches the
 * backend: ≤0 means mark-only (no `auto_expire_at`).
 */
export type SensitiveExpireDisplay =
  | { kind: "never" }
  | { kind: "seconds"; seconds: number }
  | { kind: "minutes"; minutes: number }
  | { kind: "compound"; minutes: number; seconds: number };

export function sensitiveExpireDisplay(seconds: number): SensitiveExpireDisplay {
  if (!Number.isFinite(seconds) || seconds <= 0) return { kind: "never" };
  const s = Math.trunc(seconds);
  if (s < 60) return { kind: "seconds", seconds: s };
  const minutes = Math.floor(s / 60);
  const rem = s % 60;
  if (rem === 0) return { kind: "minutes", minutes };
  return { kind: "compound", minutes, seconds: rem };
}
