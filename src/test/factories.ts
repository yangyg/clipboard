/**
 * Shared test factories. Each spec used to define its own `makeRecord` copy
 * with slightly different field sets — a new required field on
 * `ClipboardRecord` meant patching three files by hand. Single source of truth.
 */
import type { ClipboardRecord } from "../types";

export function makeRecord(overrides: Partial<ClipboardRecord> = {}): ClipboardRecord {
  return {
    id: 1,
    content: "hello",
    content_type: "text",
    source_app: "test.exe",
    source_window: "Test",
    hash: "abc",
    copy_count: 0,
    is_favorite: false,
    is_pinned: false,
    is_sensitive: false,
    is_trashed: false,
    auto_expire_at: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    tags: [],
    alias: "",
    ...overrides,
  };
}
