import type { FilterTab } from "../types";

export type { FilterTab } from "../types";

export const FILTER_DEFINITIONS = [
  { key: "all", icon: "clipboard", labelKey: "category.all", filterLabelKey: "filter.all", color: undefined },
  { key: "text", icon: "type", labelKey: "category.text", filterLabelKey: "filter.text", color: "var(--type-text)" },
  { key: "image", icon: "image", labelKey: "category.image", filterLabelKey: "filter.image", color: "var(--type-image)" },
  { key: "file", icon: "file", labelKey: "category.file", filterLabelKey: "filter.file", color: "var(--type-file)" },
  { key: "link", icon: "link", labelKey: "category.link", filterLabelKey: "filter.link", color: "var(--type-link)" },
  { key: "code", icon: "code", labelKey: "category.code", filterLabelKey: "filter.code", color: "var(--type-code)" },
  { key: "favorites", icon: "star", labelKey: "category.favorites", filterLabelKey: "filter.favorites", color: "var(--warning)" },
] as const satisfies ReadonlyArray<{
  key: FilterTab;
  icon: string;
  labelKey: string;
  filterLabelKey: string;
  color?: string;
}>;

export const CONTENT_FILTER_DEFINITIONS = FILTER_DEFINITIONS.filter(
  (definition) => definition.key !== "all" && definition.key !== "favorites",
);

export function isFilterTab(value: string): value is FilterTab {
  return FILTER_DEFINITIONS.some((definition) => definition.key === value);
}
