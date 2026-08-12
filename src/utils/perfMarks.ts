/**
 * Opt-in performance instrumentation for hot UI paths (see docs/perf.md).
 *
 * `performance.mark` is sub-microsecond, so marks are always recorded;
 * measurements are logged to the console at debug level only, keeping normal
 * output clean. Use in the DevTools console / log viewer to verify latency
 * targets (e.g. panel show ≤ 150ms, search round-trip).
 */

const reportedOnce = new Set<string>();

export function perfMark(name: string): void {
  if (typeof performance === "undefined") return;
  performance.mark(name);
}

/** Measure from an existing start mark to now and log at debug level. */
export function perfMeasure(name: string, startMark: string): number | null {
  if (typeof performance === "undefined") return null;
  try {
    const m = performance.measure(name, startMark);
    const ms = m.duration;
    console.debug(`[perf] ${name}: ${ms.toFixed(1)}ms`);
    return ms;
  } catch {
    return null;
  }
}

/** Same as perfMeasure, but reports only the first occurrence per name. */
export function perfMeasureOnce(name: string, startMark: string): number | null {
  if (reportedOnce.has(name)) return null;
  reportedOnce.add(name);
  return perfMeasure(name, startMark);
}
