import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import type { VueWrapper } from "@vue/test-utils";
import SettingsPrivacy from "./SettingsPrivacy.vue";
import { mountWithPlugins } from "../../test/mount";
import { useSettingsStore } from "../../stores/settings";

function mountPrivacy() {
  return mountWithPlugins(SettingsPrivacy, {
    attachTo: document.body,
  }) as VueWrapper<any>;
}

function findInput(wrapper: VueWrapper<any>) {
  return wrapper.find("input.ignore-input");
}

function findAddBtn(wrapper: VueWrapper<any>) {
  return wrapper.find(".ignore-add-row .btn-primary");
}

async function seedIgnoredApps(apps: string[]) {
  useSettingsStore().updateSetting("ignored_apps", apps);
  await nextTick();
}

describe("SettingsPrivacy ignored-app add row", () => {
  it("disables the add button while the input is empty or whitespace", async () => {
    const wrapper = mountPrivacy();
    expect(findAddBtn(wrapper).attributes("disabled")).toBeDefined();

    await findInput(wrapper).setValue("   ");
    expect(findAddBtn(wrapper).attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("enables the add button once a name is typed", async () => {
    const wrapper = mountPrivacy();
    await findInput(wrapper).setValue("Foo.exe");
    expect(findAddBtn(wrapper).attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("adds the app, clears the box and keeps the input focused", async () => {
    const wrapper = mountPrivacy();
    await seedIgnoredApps([]);

    await findInput(wrapper).setValue("Foo.exe");
    (findInput(wrapper).element as HTMLInputElement).focus();
    await findAddBtn(wrapper).trigger("click");

    expect(useSettingsStore().settings.ignored_apps).toEqual(["Foo.exe"]);
    expect((findInput(wrapper).element as HTMLInputElement).value).toBe("");
    // Ready for the next entry without re-clicking the box.
    expect(document.activeElement).toBe(findInput(wrapper).element);
    wrapper.unmount();
  });

  it("rejects a duplicate regardless of case", async () => {
    const wrapper = mountPrivacy();
    await seedIgnoredApps(["Notepad.exe"]);

    await findInput(wrapper).setValue("notepad.EXE");
    await findAddBtn(wrapper).trigger("click");

    expect(useSettingsStore().settings.ignored_apps).toEqual(["Notepad.exe"]);
    wrapper.unmount();
  });

  it("treats the extension-less name as a duplicate of the .exe entry", async () => {
    const wrapper = mountPrivacy();
    await seedIgnoredApps(["Notepad.exe"]);

    await findInput(wrapper).setValue("notepad");
    await findAddBtn(wrapper).trigger("click");

    expect(useSettingsStore().settings.ignored_apps).toEqual(["Notepad.exe"]);
    wrapper.unmount();
  });

  it("does not add on Enter while empty", async () => {
    const wrapper = mountPrivacy();
    await seedIgnoredApps([]);

    await findInput(wrapper).trigger("keydown", { key: "Enter" });

    expect(useSettingsStore().settings.ignored_apps).toEqual([]);
    wrapper.unmount();
  });
});
