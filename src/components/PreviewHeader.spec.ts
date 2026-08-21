import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { makeRecord } from "../test/factories";
import { mountWithPlugins } from "../test/mount";
import { useSettingsStore } from "../stores/settings";
import PreviewHeader from "./PreviewHeader.vue";

function mountHeader(record = makeRecord()) {
  return mountWithPlugins(PreviewHeader, {
    props: {
      record,
      typeLabel: "纯文本",
      recordAlias: record.alias || "",
      formatDateTime: () => "now",
    },
    attachTo: document.body,
  });
}

describe("PreviewHeader on-demand AI", () => {
  it("hides the sparkles button until AI is enabled", () => {
    const wrapper = mountHeader();
    expect(wrapper.find(".preview-ai-btn").exists()).toBe(false);
    wrapper.unmount();
  });

  it("shows sparkles for eligible text records when AI is on", async () => {
    const wrapper = mountHeader();
    useSettingsStore().updateSetting("enable_ai", true);
    await nextTick();
    expect(wrapper.find(".preview-ai-btn").exists()).toBe(true);
    wrapper.unmount();
  });

  it("hides sparkles for images and sensitive records", async () => {
    const wrapper = mountHeader(makeRecord({ content_type: "image" }));
    useSettingsStore().updateSetting("enable_ai", true);
    await nextTick();
    expect(wrapper.find(".preview-ai-btn").exists()).toBe(false);
    wrapper.unmount();

    const sensitive = mountHeader(makeRecord({ is_sensitive: true }));
    useSettingsStore().updateSetting("enable_ai", true);
    await nextTick();
    expect(sensitive.find(".preview-ai-btn").exists()).toBe(false);
    sensitive.unmount();
  });
});
