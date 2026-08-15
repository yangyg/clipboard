/** Center-column (list + preview) below this width cannot keep a side-by-side column. */
export const PREVIEW_DRAWER_BREAKPOINT = 560;

/** List column never shrinks below this when preview is a side-by-side column. */
export const LIST_MIN = 280;

export const PREVIEW_MIN = 280;
export const PREVIEW_MAX = 520;
export const PREVIEW_DEFAULT = 360;

/** Classic three-column list width bounds. */
export const LIST_COL_MIN = 280;
export const LIST_COL_MAX = 720;
export const LIST_COL_DEFAULT = 400;

export type PreviewLayoutPref = "columns" | "on_demand" | "drawer";

export type PreviewChrome =
  | { kind: "hidden" }
  | { kind: "column"; sizing: "flex" | "fixed" }
  | { kind: "drawer" };

export function normalizePreviewLayoutPref(raw: unknown): PreviewLayoutPref {
  if (raw === "columns" || raw === "on_demand" || raw === "drawer") return raw;
  return "on_demand";
}

/**
 * Resolve how the preview host should render.
 *
 * `wrapperWidth === 0` means the host has not been measured yet — do not
 * flash the drawer on first paint.
 */
export function resolvePreviewChrome(
  pref: PreviewLayoutPref,
  hasSelection: boolean,
  batchMode: boolean,
  wrapperWidth: number,
): PreviewChrome {
  if (batchMode) return { kind: "hidden" };

  const tooNarrow = wrapperWidth > 0 && wrapperWidth < PREVIEW_DRAWER_BREAKPOINT;

  if (pref === "drawer") {
    return hasSelection ? { kind: "drawer" } : { kind: "hidden" };
  }

  if (pref === "columns") {
    if (tooNarrow) return hasSelection ? { kind: "drawer" } : { kind: "hidden" };
    return { kind: "column", sizing: "flex" };
  }

  if (!hasSelection) return { kind: "hidden" };
  if (tooNarrow) return { kind: "drawer" };
  return { kind: "column", sizing: "fixed" };
}

/** Cap stored preview width so the list keeps at least `LIST_MIN`. */
export function clampPreviewWidth(stored: number, wrapperWidth: number): number {
  const bounded = Math.min(PREVIEW_MAX, Math.max(PREVIEW_MIN, stored));
  if (wrapperWidth <= 0) return bounded;
  const maxFit = Math.max(PREVIEW_MIN, wrapperWidth - LIST_MIN);
  return Math.min(bounded, maxFit);
}
