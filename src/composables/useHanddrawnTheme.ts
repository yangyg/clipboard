/**
 * Reactive "hand-drawn theme active" flag for AppIcon.
 *
 * The hand-drawn theme swaps clean Lucide icons for the hand-drawn
 * @sketchyicons/vue set. The theme class lives on <body> and is toggled by
 * both the settings store and the tray-menu chrome, so we watch the body
 * class with a MutationObserver (no store dependency) to stay reactive in
 * every window.
 */
import { ref } from "vue";

const isHanddrawn = ref(false);
let started = false;

function syncHanddrawn() {
  const cls = document.body.classList;
  isHanddrawn.value =
    cls.contains("handdrawn-theme") || cls.contains("handdrawn-light-theme");
}

export function useHanddrawnTheme() {
  if (!started) {
    started = true;
    syncHanddrawn();
    if (typeof MutationObserver !== "undefined") {
      const observer = new MutationObserver(syncHanddrawn);
      observer.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    }
  }
  return isHanddrawn;
}
