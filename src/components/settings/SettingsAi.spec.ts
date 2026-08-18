import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import type { VueWrapper } from "@vue/test-utils";
import SettingsAi from "./SettingsAi.vue";
import { mountWithPlugins } from "../../test/mount";
import { useSettingsStore } from "../../stores/settings";

function mountAi() {
  return mountWithPlugins(SettingsAi, {
    attachTo: document.body,
  }) as VueWrapper<any>;
}

async function enableAi() {
  useSettingsStore().updateSetting("enable_ai", true);
  await nextTick();
}

function findAddInput(wrapper: VueWrapper<any>) {
  return wrapper.find("input.ai-model-add-input");
}

function findAddBtn(wrapper: VueWrapper<any>) {
  return wrapper.find(".ai-model-add-btn");
}

describe("SettingsAi model list", () => {
  it("disables add while the name is empty or whitespace", async () => {
    const wrapper = mountAi();
    await enableAi();
    expect(findAddBtn(wrapper).attributes("disabled")).toBeDefined();

    await findAddInput(wrapper).setValue("   ");
    expect(findAddBtn(wrapper).attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("adds a model without switching the current selection", async () => {
    const wrapper = mountAi();
    await enableAi();
    const store = useSettingsStore();
    expect(store.settings.ai_model).toBe("gpt-4o-mini");

    await findAddInput(wrapper).setValue("deepseek-chat");
    await findAddBtn(wrapper).trigger("click");

    expect(store.settings.ai_models).toEqual(["gpt-4o-mini", "deepseek-chat"]);
    expect(store.settings.ai_model).toBe("gpt-4o-mini");
    expect((findAddInput(wrapper).element as HTMLInputElement).value).toBe("");
    wrapper.unmount();
  });

  it("selects a radio as the current model", async () => {
    const wrapper = mountAi();
    await enableAi();
    const store = useSettingsStore();
    store.updateSetting("ai_models", ["gpt-4o-mini", "llama3"]);
    await nextTick();

    const radios = wrapper.findAll('[role="radio"]');
    await radios[1].trigger("click");
    expect(store.settings.ai_model).toBe("llama3");
    expect(radios[1].attributes("aria-checked")).toBe("true");
    wrapper.unmount();
  });

  it("disables delete on the last remaining model", async () => {
    const wrapper = mountAi();
    await enableAi();
    expect(wrapper.find(".ai-model-remove").attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("selects the first remaining model when the current one is removed", async () => {
    const wrapper = mountAi();
    await enableAi();
    const store = useSettingsStore();
    store.updateSetting("ai_models", ["gpt-4o-mini", "llama3"]);
    store.updateSetting("ai_model", "llama3");
    await nextTick();

    const removeBtns = wrapper.findAll(".ai-model-remove");
    await removeBtns[1].trigger("click");
    expect(store.settings.ai_models).toEqual(["gpt-4o-mini"]);
    expect(store.settings.ai_model).toBe("gpt-4o-mini");
    wrapper.unmount();
  });

  it("rejects a duplicate name", async () => {
    const wrapper = mountAi();
    await enableAi();
    await findAddInput(wrapper).setValue("gpt-4o-mini");
    await findAddBtn(wrapper).trigger("click");
    expect(useSettingsStore().settings.ai_models).toEqual(["gpt-4o-mini"]);
    wrapper.unmount();
  });

  it("appends a preset model and selects it as current", async () => {
    const wrapper = mountAi();
    await enableAi();
    const chips = wrapper.findAll(".ai-preset-chip");
    await chips[1].trigger("click");
    const store = useSettingsStore();
    expect(store.settings.ai_base_url).toBe("https://api.deepseek.com/v1");
    expect(store.settings.ai_models).toEqual(["gpt-4o-mini", "deepseek-chat"]);
    expect(store.settings.ai_model).toBe("deepseek-chat");
    wrapper.unmount();
  });
});
