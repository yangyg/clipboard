/**
 * WebDAV sync result → human-readable success message.
 * `t` is the vue-i18n translate function (parameterized so this stays a
 * dependency-free util module). Content clauses are always shown; tag and
 * media clauses are omitted when their counters are zero.
 */
import type { TranslateFn } from "./recordFormatting";
import type { WebDavSyncResult } from "../types";

export type WebDavAction = "pull" | "push" | "sync";

function tagSummary(tags: number, t: TranslateFn): string {
  return tags > 0 ? t('settings.sync.resultTags', { count: tags }) : "";
}

function mediaSummary(result: WebDavSyncResult, t: TranslateFn): string {
  const parts: string[] = [];
  if (result.media_downloaded > 0) {
    parts.push(t('settings.sync.resultMediaDownload', { count: result.media_downloaded }));
  }
  if (result.media_uploaded > 0) {
    parts.push(t('settings.sync.resultMediaUpload', { count: result.media_uploaded }));
  }
  if (result.media_skipped > 0) {
    parts.push(t('settings.sync.resultMediaSkip', { count: result.media_skipped }));
  }
  return parts.join("，");
}

export function formatWebDavResult(
  result: WebDavSyncResult,
  action: WebDavAction,
  t: TranslateFn,
): string {
  let base: string;
  if (action === "pull") {
    base = t('settings.sync.pullResult', { pulled: result.pulled, merged: result.merged });
  } else if (action === "push") {
    base = t('settings.sync.pushResult', { pushed: result.pushed });
  } else {
    base = t('settings.sync.syncResult', {
      pulled: result.pulled,
      merged: result.merged,
      pushed: result.pushed,
    });
  }
  // Sync combines both directions; pull/push surface their own direction.
  const tags = action === "pull"
    ? result.tags_pulled
    : action === "push"
      ? result.tags_pushed
      : result.tags_pulled + result.tags_pushed;
  const clauses = [base, tagSummary(tags, t), mediaSummary(result, t)].filter(Boolean);
  return clauses.join("；");
}
