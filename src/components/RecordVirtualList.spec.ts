import { describe, expect, it, beforeEach, vi } from "vitest";
import { nextTick } from "vue";
import { mountWithPlugins } from "../test/mount";
import { useClipboardStore } from "../stores/clipboard";
import { makeRecord } from "../test/factories";
import RecordVirtualList from "./RecordVirtualList.vue";

const props = {
  layout: "list" as const,
  gridCols: 2,
  displayItems: [],
  padTop: 0,
  padBottom: 0,
  reloading: false,
  fadeArmed: false,
  fadeOn: false,
  scrollEl: () => {},
  leavingIds: new Set<number>(),
  sourceOverrides: {},
  activeDescendantId: undefined,
  isPinned: () => true,
  isOptionTabbable: () => false,
  measureRow: () => {},
  setPinnedBlockEl: () => {},
};

type IoHandle = {
  cb: IntersectionObserverCallback;
  observe: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
};

let ioHandles: IoHandle[] = [];

function fireIntersecting(intersecting: boolean) {
  const h = ioHandles[ioHandles.length - 1];
  if (!h) return;
  h.cb(
    [
      {
        isIntersecting: intersecting,
        intersectionRatio: intersecting ? 1 : 0,
      } as IntersectionObserverEntry,
    ],
    h as unknown as IntersectionObserver,
  );
}

describe("RecordVirtualList pinned section", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
    ioHandles = [];
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        observe = vi.fn();
        disconnect = vi.fn();
        unobserve = vi.fn();
        constructor(cb: IntersectionObserverCallback) {
          const handle: IoHandle = {
            cb,
            observe: this.observe,
            disconnect: this.disconnect,
          };
          ioHandles.push(handle);
        }
      },
    );
  });

  it("renders the pinned header inside the scroll list", async () => {
    const wrapper = mountWithPlugins(RecordVirtualList, { props });
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, is_pinned: true }),
      makeRecord({ id: 2, is_pinned: true }),
    ];
    await nextTick();

    const btn = wrapper.get(".record-list .section-label");
    expect(btn.attributes("aria-expanded")).toBe("true");
    expect(btn.text()).toContain("置顶");
    expect(btn.text()).toContain("2");
    expect(wrapper.find(".pinned-dock").exists()).toBe(false);

    await btn.trigger("click");
    expect(store.pinnedCollapsed).toBe(true);
    expect(btn.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(".record-list .section-label").exists()).toBe(true);
  });

  it("docks the whole pinned block after it leaves the viewport and restores on intersect", async () => {
    const wrapper = mountWithPlugins(RecordVirtualList, { props });
    useClipboardStore().records = [
      makeRecord({ id: 1, is_pinned: true }),
      makeRecord({ id: 3, is_pinned: false }),
    ];
    await nextTick();
    await nextTick();

    fireIntersecting(false);
    await nextTick();
    expect(wrapper.find(".pinned-dock").exists()).toBe(true);
    expect(wrapper.find(".pinned-block").classes()).toContain("is-docked");

    const dockBtn = wrapper.get(".pinned-dock .section-label");
    await dockBtn.trigger("click");
    expect(useClipboardStore().pinnedCollapsed).toBe(true);

    fireIntersecting(true);
    await nextTick();
    expect(wrapper.find(".pinned-dock").exists()).toBe(false);
    expect(wrapper.find(".pinned-block").classes()).not.toContain("is-docked");
  });

  it("hides the pinned header when nothing is pinned", async () => {
    const wrapper = mountWithPlugins(RecordVirtualList, { props });
    useClipboardStore().records = [makeRecord({ id: 3, is_pinned: false })];
    await nextTick();
    expect(wrapper.find(".section-label").exists()).toBe(false);
  });
});
