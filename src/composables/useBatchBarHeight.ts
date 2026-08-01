import { ref, watch, onUnmounted, type Ref } from "vue";

/**
 * Tracks the rendered height of the floating batch bar so hosts can reserve
 * space (top padding) without the bar participating in document flow.
 * Handles font scaling / i18n wrapping via ResizeObserver.
 */
export function useBatchBarHeight(holderRef: Ref<HTMLElement | null>) {
  const height = ref(0);
  let ro: ResizeObserver | null = null;

  watch(holderRef, (el) => {
    ro?.disconnect();
    ro = null;
    if (!el) return;
    const update = () => {
      height.value = el.offsetHeight;
    };
    update();
    ro = new ResizeObserver(update);
    ro.observe(el);
  });

  onUnmounted(() => ro?.disconnect());

  return { height };
}
