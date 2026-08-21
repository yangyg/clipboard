import type { ClipboardRecord, FeatureFlags, Settings } from "../types";

export type AiEnrichMode = "summary" | "tags";

export interface AiEnrichOutcome {
  alias?: string | null;
  tags?: string[] | null;
}

/** Text-ish, active, non-sensitive records may be sent to the model. */
export function isOnDemandAiRecord(record: Pick<ClipboardRecord, "is_trashed" | "is_sensitive" | "content_type">): boolean {
  if (record.is_trashed || record.is_sensitive) return false;
  return record.content_type === "text" || record.content_type === "code" || record.content_type === "link";
}

/** Dual gate: capability on and the user has flipped the runtime switch. */
export function isOnDemandAiEnabled(
  settings: Pick<Settings, "enable_ai"> & { features: Pick<FeatureFlags, "ai"> },
): boolean {
  return settings.features.ai !== false && settings.enable_ai;
}

/** Which on-demand actions to offer. Empty = hide the sparkles entry entirely. */
export function onDemandAiActions(
  record: Pick<ClipboardRecord, "is_trashed" | "is_sensitive" | "content_type">,
  settings: Pick<Settings, "enable_ai"> & { features: Pick<FeatureFlags, "ai" | "tags"> },
): AiEnrichMode[] {
  if (!isOnDemandAiEnabled(settings) || !isOnDemandAiRecord(record)) return [];
  const actions: AiEnrichMode[] = ["summary"];
  if (settings.features.tags !== false) actions.push("tags");
  return actions;
}
