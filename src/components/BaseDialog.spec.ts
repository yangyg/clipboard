import { describe, it, expect } from "vitest";
import BaseDialog from "./BaseDialog.vue";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

/**
 * BaseDialog wraps its content in <Teleport to="body">. With @vue/test-utils
 * we stub Teleport so the content stays inside the wrapper tree.
 */
const stubs = { teleport: true };

describe("BaseDialog", () => {
  it("does not render content when open is false", () => {
    const wrapper = mount(BaseDialog, {
      props: { open: false },
      slots: { default: "<p>Dialog body</p>" },
      global: { stubs },
    });
    expect(wrapper.find(".dialog-card").exists()).toBe(false);
  });

  it("renders content inside dialog card when open is true", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true },
      slots: { default: "<p>Dialog body</p>" },
      global: { stubs },
    });
    await nextTick();
    expect(wrapper.find(".dialog-card").exists()).toBe(true);
    expect(wrapper.find("p").text()).toBe("Dialog body");
  });

  it("sets the correct ARIA role", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true, role: "alertdialog" },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    expect(wrapper.find(".dialog-card").attributes("role")).toBe("alertdialog");
  });

  it("emits close on Escape (window keydown, single handler)", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    // Escape is handled once at the window (capture) listener while open —
    // the card-level handler only deals with Tab focus trapping. The test
    // wrapper is a detached tree, so dispatch on window directly.
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("does not emit close on Escape when closed", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: false },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("close")).toBeFalsy();
  });

  it("emits close when overlay is clicked (closeOnOverlay=true)", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    await wrapper.find(".dialog-overlay").trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("does not emit close on Escape when closeOnEscape=false", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true, closeOnEscape: false },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("close")).toBeFalsy();
  });

  it("does not emit close on overlay click when closeOnOverlay=false", async () => {
    const wrapper = mount(BaseDialog, {
      props: { open: true, closeOnOverlay: false },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    await wrapper.find(".dialog-overlay").trigger("click");
    expect(wrapper.emitted("close")).toBeFalsy();
  });

  it("sets aria-modal and labelledby/describedby attributes", async () => {
    const wrapper = mount(BaseDialog, {
      props: {
        open: true,
        labelledBy: "title-id",
        describedBy: "desc-id",
      },
      slots: { default: "<p>body</p>" },
      global: { stubs },
    });
    await nextTick();
    const card = wrapper.find(".dialog-card");
    expect(card.attributes("aria-modal")).toBe("true");
    expect(card.attributes("aria-labelledby")).toBe("title-id");
    expect(card.attributes("aria-describedby")).toBe("desc-id");
  });
});
