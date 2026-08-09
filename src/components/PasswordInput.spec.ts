import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import type { VueWrapper } from "@vue/test-utils";
import PasswordInput from "./PasswordInput.vue";
import { mountWithPlugins } from "../test/mount";

function mountPassword(props: Record<string, unknown> = {}, attach = false) {
  const wrapper = mountWithPlugins(PasswordInput, {
    props: { modelValue: "", ...props },
    ...(attach ? { attachTo: document.body } : {}),
  });
  return wrapper as VueWrapper<any>;
}

function findToggle(wrapper: VueWrapper<any>) {
  return wrapper.find(".input-trailing-btn");
}

function inputOf(wrapper: VueWrapper<any>) {
  return wrapper.find("input").element as HTMLInputElement;
}

describe("PasswordInput visibility toggle", () => {
  it("defaults to masked (type=password)", () => {
    const wrapper = mountPassword({ modelValue: "s3cret" });
    expect(inputOf(wrapper).type).toBe("password");
  });

  it("shows a toggle labelled 显示密码 with aria-pressed=false", () => {
    const wrapper = mountPassword({ modelValue: "s3cret" });
    const btn = findToggle(wrapper);
    expect(btn.exists()).toBe(true);
    expect(btn.attributes("aria-label")).toBe("显示密码");
    expect(btn.attributes("aria-pressed")).toBe("false");
    // Keyboard reachable — no negative tabindex.
    expect(btn.attributes("tabindex")).toBeUndefined();
  });

  it("reveals the password on click and relabels to 隐藏密码", async () => {
    const wrapper = mountPassword({ modelValue: "s3cret" });
    await findToggle(wrapper).trigger("click");
    await nextTick();
    expect(inputOf(wrapper).type).toBe("text");
    const btn = findToggle(wrapper);
    expect(btn.attributes("aria-label")).toBe("隐藏密码");
    expect(btn.attributes("aria-pressed")).toBe("true");
  });

  it("masks again on the second click", async () => {
    const wrapper = mountPassword({ modelValue: "s3cret" });
    await findToggle(wrapper).trigger("click");
    await nextTick();
    await findToggle(wrapper).trigger("click");
    await nextTick();
    expect(inputOf(wrapper).type).toBe("password");
    expect(findToggle(wrapper).attributes("aria-label")).toBe("显示密码");
  });

  it("keeps the value and the input focus while toggling", async () => {
    const wrapper = mountPassword({ modelValue: "s3cret" }, true);
    const input = inputOf(wrapper);
    input.focus();
    expect(document.activeElement).toBe(input);

    await findToggle(wrapper).trigger("click");
    await nextTick();

    expect(inputOf(wrapper).value).toBe("s3cret");
    // Focus never leaves the field during the type swap.
    expect(document.activeElement).toBe(inputOf(wrapper));
    wrapper.unmount();
  });

  it("emits update:modelValue while typing", async () => {
    const wrapper = mountPassword();
    await wrapper.find("input").setValue("pw123");
    expect(wrapper.emitted("update:modelValue")).toEqual([["pw123"]]);
  });

  it("hides the toggle when disabled", () => {
    const wrapper = mountPassword({ modelValue: "s3cret", disabled: true });
    expect(findToggle(wrapper).exists()).toBe(false);
    expect(inputOf(wrapper).disabled).toBe(true);
  });

  it("hides the toggle when readonly", () => {
    const wrapper = mountPassword({ modelValue: "s3cret", readonly: true });
    expect(findToggle(wrapper).exists()).toBe(false);
    expect(inputOf(wrapper).readOnly).toBe(true);
  });

  it("passes through attrs (autocomplete / aria-label / id) to the native input", () => {
    const wrapper = mountPassword({
      modelValue: "",
      autocomplete: "current-password",
      id: "pw-input",
      "aria-label": "密码",
    });
    const input = wrapper.find("input");
    expect(input.attributes("autocomplete")).toBe("current-password");
    expect(input.attributes("id")).toBe("pw-input");
    expect(input.attributes("aria-label")).toBe("密码");
  });
});
