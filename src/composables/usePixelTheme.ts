/**
 * Reactive "special icon theme active" flags for AppIcon.
 *
 * The pixel theme swaps clean Lucide icons for a hand-authored 8-bit set; the
 * hand-drawn theme swaps them for @sketchyicons/vue. The theme class lives on
 * <body> and is toggled by both the settings store and the tray-menu chrome,
 * so we watch the body class with a MutationObserver (no store dependency) to
 * stay reactive in every window.
 */
import { ref } from "vue";

/** Class prefixes (with a `-light-theme` sibling) that toggle a swap set. */
const THEME_CLASS_PREFIXES = ["pixel", "handdrawn"] as const;
type ThemePrefix = (typeof THEME_CLASS_PREFIXES)[number];

/** Shared observer: one MutationObserver serves every prefix. */
const flags = {} as Record<ThemePrefix, ReturnType<typeof ref<boolean>>>;
for (const prefix of THEME_CLASS_PREFIXES) {
  flags[prefix] = ref(false);
}

let started = false;

function syncFlags() {
  const cls = document.body.classList;
  for (const prefix of THEME_CLASS_PREFIXES) {
    flags[prefix].value =
      cls.contains(`${prefix}-theme`) || cls.contains(`${prefix}-light-theme`);
  }
}

function useBodyThemeFlag(prefix: ThemePrefix) {
  if (!started) {
    started = true;
    syncFlags();
    if (typeof MutationObserver !== "undefined") {
      const observer = new MutationObserver(syncFlags);
      observer.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    }
  }
  return flags[prefix];
}

export function usePixelTheme() {
  return useBodyThemeFlag("pixel");
}

export function useHanddrawnTheme() {
  return useBodyThemeFlag("handdrawn");
}
