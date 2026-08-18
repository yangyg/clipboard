/** Soft cap for saved AI model names (settings JSON blob). */
export const AI_MODELS_MAX = 20;

export const DEFAULT_AI_MODEL = "gpt-4o-mini";

export type NormalizedAiModels = {
  models: string[];
  current: string;
};

/**
 * Canonicalize the AI model list + current selection.
 *
 * Upgrade JSON that only has `ai_model` (no `ai_models`) is treated as an
 * empty list so the custom current name is kept instead of being mixed with
 * the default `gpt-4o-mini`.
 */
export function normalizeAiModels(
  models: unknown,
  current: unknown,
): NormalizedAiModels {
  const currentName = typeof current === "string" ? current.trim() : "";
  const rawList = Array.isArray(models) ? models : [];
  const seen = new Set<string>();
  const list: string[] = [];
  for (const item of rawList) {
    if (typeof item !== "string") continue;
    const name = item.trim();
    if (!name || seen.has(name)) continue;
    seen.add(name);
    list.push(name);
  }
  if (currentName && !seen.has(currentName)) {
    list.unshift(currentName);
    seen.add(currentName);
  }
  if (list.length === 0) {
    list.push(DEFAULT_AI_MODEL);
  }
  const active =
    currentName && list.includes(currentName) ? currentName : list[0];
  if (list.length <= AI_MODELS_MAX) {
    return { models: list, current: active };
  }
  if (list.slice(0, AI_MODELS_MAX).includes(active)) {
    return { models: list.slice(0, AI_MODELS_MAX), current: active };
  }
  const others = list.filter((name) => name !== active);
  return { models: [active, ...others].slice(0, AI_MODELS_MAX), current: active };
}
