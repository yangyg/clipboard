/**
 * Reactive "pixel theme active" flag for AppIcon.
 *
 * The pixel theme swaps clean Lucide icons for a hand-authored 8-bit pixel
 * set. The theme class lives on <body> and is toggled by both the settings
 * store and the tray-menu chrome, so we watch the body class with a
 * MutationObserver (no store dependency) to stay reactive in every window,
 * mirroring `useHanddrawnTheme`.
 */
import { ref } from "vue";

const isPixel = ref(false);
let started = false;

function syncPixel() {
  const cls = document.body.classList;
  isPixel.value = cls.contains("pixel-theme") || cls.contains("pixel-light-theme");
}

export function usePixelTheme() {
  if (!started) {
    started = true;
    syncPixel();
    if (typeof MutationObserver !== "undefined") {
      const observer = new MutationObserver(syncPixel);
      observer.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    }
  }
  return isPixel;
}