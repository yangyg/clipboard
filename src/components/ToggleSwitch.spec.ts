import { describe, it, expect } from "vitest";
import ToggleSwitch from "./ToggleSwitch.vue";
import { mount } from "@vue/test-utils";

describe("ToggleSwitch", () => {
  it("renders with role=switch and correct aria-checked", () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false },
    });
    expect(wrapper.attributes("role")).toBe("switch");
    expect(wrapper.attributes("aria-checked")).toBe("false");
  });

  it("adds the 'on' class when modelValue is true", () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: true },
    });
    expect(wrapper.classes()).toContain("on");
  });

  it("does not have the 'on' class when modelValue is false", () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false },
    });
    expect(wrapper.classes()).not.toContain("on");
  });

  it("emits update:modelValue with toggled value on click", async () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false },
    });
    await wrapper.trigger("click");
    expect(wrapper.emitted("update:modelValue")).toEqual([[true]]);
  });

  it("emits false when toggled from true", async () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: true },
    });
    await wrapper.trigger("click");
    expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
  });

  it("emits toggle on Enter keydown", async () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false },
    });
    await wrapper.trigger("keydown.enter");
    expect(wrapper.emitted("update:modelValue")).toEqual([[true]]);
  });

  it("emits toggle on Space keydown", async () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: true },
    });
    await wrapper.trigger("keydown.space");
    expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
  });

  it("sets aria-label when provided", () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false, ariaLabel: "Dark mode" },
    });
    expect(wrapper.attributes("aria-label")).toBe("Dark mode");
  });

  it("is focusable via tabindex", () => {
    const wrapper = mount(ToggleSwitch, {
      props: { modelValue: false },
    });
    expect(wrapper.attributes("tabindex")).toBe("0");
  });
});
