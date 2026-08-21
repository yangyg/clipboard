import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { defineComponent, ref } from "vue";
import { makeRecord } from "../test/factories";
import { mountWithPlugins } from "../test/mount";
import { useSettingsStore } from "../stores/settings";
import { useRecordActions } from "./useRecordActions";
import type { ClipboardRecord } from "../types";

const Harness = defineComponent({
  props: {
    record: { type: Object, required: true },
  },
  setup(props) {
    const actions = useRecordActions({
      listRef: ref(null),
      scrollTop: ref(0),
      flatItems: () => [],
      isEmptyOrLoading: () => false,
      selectedId: () => 1,
      pinnedDocked: () => false,
    });
    actions.showContextMenu(
      { clientX: 0, clientY: 0, preventDefault() {} } as MouseEvent,
      props.record as ClipboardRecord,
    );
    return { ids: actions.contextMenuItems };
  },
  template: `<div>{{ ids.map((i) => i.id).join(",") }}</div>`,
});

describe("useRecordActions on-demand AI menu", () => {
  it("omits AI items until the runtime switch is on", () => {
    const wrapper = mountWithPlugins(Harness, {
      props: { record: makeRecord() },
    });
    expect(wrapper.text()).not.toContain("ai-summary");
    wrapper.unmount();
  });

  it("inserts summary and tags after alias when AI is enabled", async () => {
    const wrapper = mountWithPlugins(Harness, {
      props: { record: makeRecord() },
    });
    useSettingsStore().updateSetting("enable_ai", true);
    await nextTick();
    expect(wrapper.text()).toContain("alias,ai-summary,ai-tags,delete");
    wrapper.unmount();
  });

  it("omits AI items for images", async () => {
    const wrapper = mountWithPlugins(Harness, {
      props: { record: makeRecord({ content_type: "image" }) },
    });
    useSettingsStore().updateSetting("enable_ai", true);
    await nextTick();
    expect(wrapper.text()).not.toContain("ai-summary");
    wrapper.unmount();
  });
});
