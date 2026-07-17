import { convertFileSrc } from "@tauri-apps/api/core";
import type { ClipboardRecord } from "../types";

/** Asset URL for a local absolute path (Tauri asset protocol). */
export function fileSrc(absPath: string | null | undefined): string | null {
  if (!absPath) return null;
  try {
    return convertFileSrc(absPath);
  } catch {
    return null;
  }
}

/** List thumbnail: prefer thumb, fall back to full media, then legacy base64 content. */
export function recordThumbSrc(record: ClipboardRecord): string | null {
  return (
    fileSrc(record.thumb_abs) ??
    fileSrc(record.media_abs) ??
    legacyBase64Src(record)
  );
}

/** Preview: prefer full media, then thumb, then legacy base64. */
export function recordMediaSrc(record: ClipboardRecord): string | null {
  return (
    fileSrc(record.media_abs) ??
    fileSrc(record.thumb_abs) ??
    legacyBase64Src(record)
  );
}

function legacyBase64Src(record: ClipboardRecord): string | null {
  // Old records stored PNG as base64 in content
  if (
    record.content_type === "image" &&
    record.content &&
    !record.content.startsWith("[image") &&
    record.content.length > 64
  ) {
    return `data:image/png;base64,${record.content}`;
  }
  return null;
}
