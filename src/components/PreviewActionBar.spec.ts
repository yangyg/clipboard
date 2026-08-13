import { describe, expect, it } from "vitest";
import { makeRecord } from "../test/factories";
import { mountWithPlugins } from "../test/mount";
import PreviewActionBar from "./PreviewActionBar.vue";

describe("PreviewActionBar", () => {
  it("emits paste and pin actions for active records", async () => {
    const wrapper = mountWithPlugins(PreviewActionBar, {
      props: { record: makeRecord(), pinnedDisplay: false },
    });

    await wrapper.find(".action-primary").trigger("click");
    await wrapper.find(".action-pin").trigger("click");

    expect(wrapper.emitted("paste")).toHaveLength(1);
    expect(wrapper.emitted("pin")).toHaveLength(1);
  });

  it("renders restore actions for trashed records", async () => {
    const wrapper = mountWithPlugins(PreviewActionBar, {
      props: { record: makeRecord({ is_trashed: true }), pinnedDisplay: false },
    });

    await wrapper.find(".action-primary").trigger("click");

    expect(wrapper.emitted("restore")).toHaveLength(1);
    expect(wrapper.find(".trash-actions").exists()).toBe(true);
  });
});
