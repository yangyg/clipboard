import { onUnmounted, ref, type Ref } from "vue";

/**
 * Reactive matchMedia query. Keep tooltips/behavioral choices in sync with the
 * responsive breakpoints in the component styles (e.g. the ≤720px icon rail).
 */
export function useMediaQuery(query: string): Ref<boolean> {
  const mq = window.matchMedia(query);
  const matches = ref(mq.matches);
  const onChange = (event: MediaQueryListEvent) => {
    matches.value = event.matches;
  };
  mq.addEventListener("change", onChange);
  onUnmounted(() => mq.removeEventListener("change", onChange));
  return matches;
}