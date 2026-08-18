import { afterEach, describe, expect, it, vi } from "vitest";
import { computed, defineComponent, nextTick, ref } from "vue";
import type { ClipboardRecord } from "../types";
import { makeRecord } from "../test/factories";
import { mountWithPlugins } from "../test/mount";
import { useExpireCountdown } from "./useExpireCountdown";

const NOW = "2026-08-18T10:00:00.000Z";

function mountCountdown(initial: ClipboardRecord) {
  const rec = ref<ClipboardRecord | null>(initial);
  const Comp = defineComponent({
    setup() {
      return useExpireCountdown(computed(() => rec.value));
    },
    template: `<span class="text">{{ expireText }}</span><span class="title">{{ expireTitle }}</span>`,
  });
  const wrapper = mountWithPlugins(Comp);
  return { wrapper, rec };
}

describe("useExpireCountdown", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("shows 已过期 for an unprotected past timestamp and does not start an interval", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(NOW));
    const setIntervalSpy = vi.spyOn(globalThis, "setInterval");

    const { wrapper } = mountCountdown(
      makeRecord({ auto_expire_at: "2000-01-01T00:00:00Z" }),
    );

    expect(wrapper.find(".text").text()).toBe("已过期");
    expect(wrapper.find(".title").text()).toBe("");
    expect(setIntervalSpy).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("shows protected countdown copy while pinned and still in the future", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(NOW));

    const { wrapper } = mountCountdown(
      makeRecord({
        auto_expire_at: "2026-08-18T10:00:30.000Z",
        is_pinned: true,
      }),
    );

    expect(wrapper.find(".text").text()).toBe("30s 后到期，不会自动删除");
    expect(wrapper.find(".title").text()).toBe("");
    wrapper.unmount();
  });

  it("shows kept-by-protection copy and a title after expiry", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(NOW));

    const { wrapper } = mountCountdown(
      makeRecord({
        auto_expire_at: "2000-01-01T00:00:00Z",
        is_favorite: true,
      }),
    );

    expect(wrapper.find(".text").text()).toBe("已过期，因置顶/收藏而保留");
    expect(wrapper.find(".title").text()).toBe(
      "取消置顶或收藏后将永久删除，无法从回收站恢复",
    );
    wrapper.unmount();
  });

  it("stops the interval once the timestamp is reached", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(NOW));
    const setIntervalSpy = vi.spyOn(globalThis, "setInterval");

    const { wrapper } = mountCountdown(
      makeRecord({ auto_expire_at: "2026-08-18T10:00:02.000Z" }),
    );

    await nextTick();
    expect(wrapper.find(".text").text()).toBe("2s 后自动删除");
    expect(setIntervalSpy).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(2000);
    await nextTick();
    expect(wrapper.find(".text").text()).toBe("已过期");
    const intervalCalls = setIntervalSpy.mock.calls.length;

    await vi.advanceTimersByTimeAsync(5000);
    await nextTick();
    expect(wrapper.find(".text").text()).toBe("已过期");
    expect(setIntervalSpy.mock.calls.length).toBe(intervalCalls);
    wrapper.unmount();
  });
});
