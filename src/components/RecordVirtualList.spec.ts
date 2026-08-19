import { describe, expect, it, beforeEach } from "vitest";
import { nextTick } from "vue";
import { mountWithPlugins } from "../test/mount";
import { useClipboardStore } from "../stores/clipboard";
import { makeRecord } from "../test/factories";
import RecordVirtualList from "./RecordVirtualList.vue";
import type { WindowItem } from "../composables/useVirtualList";

const labelItem: WindowItem = {
  key: "pinned-label",
  type: "label",
  height: 28,
  offset: 0,
};

const props = {
  layout: "list" as const,
  gridCols: 2,
  displayItems: [labelItem],
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
};

describe("RecordVirtualList pinned section", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
  });

  it("toggles the pinned group from the section label", async () => {
    const wrapper = mountWithPlugins(RecordVirtualList, { props });
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, is_pinned: true }),
      makeRecord({ id: 2, is_pinned: true }),
    ];
    await nextTick();

    const btn = wrapper.get(".section-label");
    expect(btn.attributes("aria-expanded")).toBe("true");
    expect(btn.text()).toContain("置顶");
    expect(btn.text()).toContain("2");

    await btn.trigger("click");
    expect(store.pinnedCollapsed).toBe(true);
    expect(btn.attributes("aria-expanded")).toBe("false");
    expect(btn.attributes("aria-label")).toBe("展开置顶");
  });
});
