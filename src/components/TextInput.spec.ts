import { describe, expect, it } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import TextInput from "./TextInput.vue";
import { mountWithPlugins } from "../test/mount";

function mountInput(props: Record<string, unknown> = {}, attach = false) {
  const wrapper = mountWithPlugins(TextInput, {
    props: { modelValue: "", ...props },
    ...(attach ? { attachTo: document.body } : {}),
  });
  return wrapper as VueWrapper<any>;
}

function findClearBtn(wrapper: VueWrapper<any>) {
  return wrapper.find(".input-trailing-btn");
}

describe("TextInput clear button", () => {
  it("hides the clear button when the value is empty", () => {
    const wrapper = mountInput({ modelValue: "" });
    expect(findClearBtn(wrapper).exists()).toBe(false);
  });

  it("shows the clear button when the value is non-empty", () => {
    const wrapper = mountInput({ modelValue: "abc" });
    expect(findClearBtn(wrapper).exists()).toBe(true);
  });

  it("hides the clear button when disabled, even with a value", () => {
    const wrapper = mountInput({ modelValue: "abc", disabled: true });
    expect(findClearBtn(wrapper).exists()).toBe(false);
    expect(wrapper.find("input").attributes("disabled")).toBeDefined();
  });

  it("hides the clear button when readonly, even with a value", () => {
    const wrapper = mountInput({ modelValue: "abc", readonly: true });
    expect(findClearBtn(wrapper).exists()).toBe(false);
    expect(wrapper.find("input").attributes("readonly")).toBeDefined();
  });

  it("exposes an accessible label and stays keyboard-focusable", () => {
    const wrapper = mountInput({ modelValue: "abc" });
    const btn = findClearBtn(wrapper);
    expect(btn.attributes("aria-label")).toBe("清空输入内容");
    expect(btn.attributes("type")).toBe("button");
    // No negative tabindex — keyboard users can reach the button.
    expect(btn.attributes("tabindex")).toBeUndefined();
  });

  it("emits update:modelValue while typing", async () => {
    const wrapper = mountInput();
    await wrapper.find("input").setValue("hi");
    expect(wrapper.emitted("update:modelValue")).toEqual([["hi"]]);
  });

  it("clears the value on click and keeps the input focused", async () => {
    const wrapper = mountInput({ modelValue: "abc" }, true);
    const input = wrapper.find("input");
    input.element.focus();
    expect(document.activeElement).toBe(input.element);

    await findClearBtn(wrapper).trigger("click");

    expect(wrapper.emitted("update:modelValue")).toEqual([[""]]);
    // Focus stays on the input so typing can continue.
    expect(document.activeElement).toBe(input.element);
    wrapper.unmount();
  });

  it("hides the clear button once the cleared value is applied", async () => {
    const wrapper = mountInput({ modelValue: "abc" });
    await findClearBtn(wrapper).trigger("click");
    await wrapper.setProps({ modelValue: "" });
    expect(findClearBtn(wrapper).exists()).toBe(false);
    expect((wrapper.find("input").element as HTMLInputElement).value).toBe("");
  });

  it("passes through attrs (placeholder / maxlength / id / aria) to the native input", () => {
    const wrapper = mountInput({
      modelValue: "",
      placeholder: "输入内容",
      maxlength: "20",
      id: "demo-input",
      "aria-label": "演示输入",
    });
    const input = wrapper.find("input");
    expect(input.attributes("placeholder")).toBe("输入内容");
    expect(input.attributes("maxlength")).toBe("20");
    expect(input.attributes("id")).toBe("demo-input");
    expect(input.attributes("aria-label")).toBe("演示输入");
    expect(input.attributes("type")).toBe("text");
  });

  it("supports type=url for text-like fields", () => {
    const wrapper = mountInput({ modelValue: "https://example.com", type: "url" });
    expect(wrapper.find("input").attributes("type")).toBe("url");
    expect(findClearBtn(wrapper).exists()).toBe(true);
  });

  it("forwards keydown listeners to the native input", async () => {
    let gotKey = "";
    const wrapper = mountWithPlugins(TextInput, {
      props: {
        modelValue: "abc",
        onKeydown: (e: KeyboardEvent) => {
          gotKey = e.key;
        },
      },
    });
    await wrapper.find("input").trigger("keydown", { key: "Enter" });
    expect(gotKey).toBe("Enter");
  });
});
