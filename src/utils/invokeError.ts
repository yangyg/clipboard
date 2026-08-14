/**
 * Map a Tauri invoke rejection to a user-facing toast string.
 * `feature disabled: {id}` becomes a named capability message; everything
 * else falls back to the generic operation-failed copy.
 */
export function humanizeInvokeError(
  err: unknown,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  const raw = err instanceof Error ? err.message : String(err);
  const match = raw.match(/feature disabled:\s*(\w+)/i);
  if (match) {
    const id = match[1];
    const labelKey = `settings.features.${id}`;
    const label = t(labelKey);
    const name = !label || label === labelKey ? id : label;
    return t("common.featureDisabled", { name });
  }
  return t("common.operationFailed");
}
