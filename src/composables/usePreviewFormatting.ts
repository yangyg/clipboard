/**
 * Preview display formatting (type label, alias, tag colors, timestamps),
 * extracted from PreviewPane.vue so the SFC script stays under 200 lines.
 */
import { computed, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ClipboardRecord, Tag } from "../types";

const TYPE_LABEL_KEYS: Record<string, string> = {
  text: "preview.typeText",
  code: "preview.typeCode",
  link: "preview.typeLink",
  image: "preview.typeImage",
  file: "preview.typeFile",
  sensitive: "preview.typeSensitive",
};

export function usePreviewFormatting(
  record: ComputedRef<ClipboardRecord | null>,
  tags: Ref<Tag[]>,
) {
  const { t } = useI18n();

  const recordAlias = computed(() => (record.value?.alias ?? "").trim());

  const typeLabel = computed(() => {
    if (!record.value) return "";
    if (record.value.is_sensitive) return t('preview.typeSensitive');
    return t(TYPE_LABEL_KEYS[record.value.content_type] ?? 'preview.typeDefault');
  });

  const tagsByName = computed(() => {
    const map = new Map<string, Tag>();
    for (const tag of tags.value) map.set(tag.name, tag);
    return map;
  });

  function normalizeHex(color: string): string {
    if (color.startsWith("#")) {
      if (color.length === 4) {
        // #abc -> #aabbcc
        return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`;
      }
      return color; // #rrggbb or #rrggbbaa
    }
    return color; // rgb()/rgba() passed through as-is
  }

  function getTagBg(tagName: string): string {
    const tag = tagsByName.value.get(tagName);
    if (!tag) return "var(--bg-surface)";
    // Normalize hex color for CSS color-mix
    return `color-mix(in srgb, ${normalizeHex(tag.color)} 10%, transparent)`;
  }

  function getTagColor(tagName: string): string {
    return tagsByName.value.get(tagName)?.color ?? "var(--text-secondary)";
  }

  function formatDateTime(iso: string): string {
    return new Date(iso).toLocaleString(undefined, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  return { recordAlias, typeLabel, tagsByName, getTagBg, getTagColor, formatDateTime };
}
