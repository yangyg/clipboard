import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { useSettingsStore } from "../stores/settings";
import SearchBar from "./SearchBar.vue";
import { mountWithPlugins } from "../test/mount";

function mountSearch() {
  return mountWithPlugins(SearchBar);
}

/** Switch the (just-created) active settings store into the given mode. */
async function setMode(mode: "full" | "icon" | "hidden") {
  useSettingsStore().updateSetting("search_mode", mode);
  await nextTick();
}

/**
 * The box is mounted with `v-show`, which toggles the inline `display` style
 * (`none` hidden, empty shown). jsdom's getComputedStyle is flaky for this, so
 * assert the inline style directly.
 */
function isBoxRevealed(wrapper: ReturnType<typeof mountSearch>): boolean {
  const row = wrapper.find(".search-row").element as HTMLElement | null;
  return row ? row.style.display !== "none" : false;
}

describe("SearchBar display modes", () => {
  it("full mode renders the search box and no trigger", () => {
    const wrapper = mountSearch();
    expect(wrapper.find(".search-box").exists()).toBe(true);
    expect(wrapper.find(".search-trigger").exists()).toBe(false);
  });

  it("icon mode shows a trigger and reveals the box on click", async () => {
    const wrapper = mountSearch();
    await setMode("icon");

    const trigger = wrapper.find(".search-trigger");
    expect(trigger.exists()).toBe(true);
    expect(isBoxRevealed(wrapper)).toBe(false);

    await trigger.trigger("click");
    await nextTick();
    expect(isBoxRevealed(wrapper)).toBe(true);
    expect(wrapper.find(".search-trigger").exists()).toBe(false);
  });

  it("icon mode collapses back on blur while empty", async () => {
    const wrapper = mountSearch();
    await setMode("icon");

    await wrapper.find(".search-trigger").trigger("click");
    await wrapper.find(".search-box").trigger("blur");
    await nextTick();

    expect(isBoxRevealed(wrapper)).toBe(false);
    expect(wrapper.find(".search-trigger").exists()).toBe(true);
  });

  it("hidden mode shows no trigger and reveals the box via the / shortcut", async () => {
    const wrapper = mountSearch();
    await setMode("hidden");

    expect(wrapper.find(".search-trigger").exists()).toBe(false);
    expect(isBoxRevealed(wrapper)).toBe(false);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "/" }));
    await nextTick();

    expect(isBoxRevealed(wrapper)).toBe(true);
  });
});

describe("SearchBar (default full) escaping", () => {
  it("frees the clear button on typed input", async () => {
    const wrapper = mountSearch();
    const input = wrapper.find(".search-box");
    await input.setValue("clip");
    expect(wrapper.find(".clear-btn").exists()).toBe(true);
    await wrapper.find(".clear-btn").trigger("click");
    expect((input.element as HTMLInputElement).value).toBe("");
  });
});