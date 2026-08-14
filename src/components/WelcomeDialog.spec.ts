import { describe, it, expect } from "vitest";
import WelcomeDialog from "./WelcomeDialog.vue";
import { mountWithPlugins } from "../test/mount";
import { nextTick } from "vue";

/**
 * WelcomeDialog renders through BaseDialog which uses <Teleport to="body">.
 * Stub Teleport so the content stays inside the wrapper tree.
 */
const stubs = { teleport: true };

describe("WelcomeDialog", () => {
  it("renders the title, steps, and start button", async () => {
    const wrapper = mountWithPlugins(WelcomeDialog, {
      props: { open: true, shortcut: "Ctrl+Shift+V" },
      global: { stubs },
    });
    await nextTick();
    expect(wrapper.find("#welcome-title").exists()).toBe(true);
    expect(wrapper.find("#welcome-desc").exists()).toBe(true);
    expect(wrapper.find("ol").findAll("li").length).toBe(3);
    expect(wrapper.find("button.btn-primary").exists()).toBe(true);
  });

  it("does not emit complete on Escape", async () => {
    const wrapper = mountWithPlugins(WelcomeDialog, {
      props: { open: true, shortcut: "Ctrl+Shift+V" },
      global: { stubs },
    });
    await nextTick();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("complete")).toBeFalsy();
  });

  it("emits 'complete' when start button is clicked", async () => {
    const wrapper = mountWithPlugins(WelcomeDialog, {
      props: { open: true, shortcut: "Ctrl+Shift+V" },
      global: { stubs },
    });
    await nextTick();
    await wrapper.find("button.btn-primary").trigger("click");
    expect(wrapper.emitted("complete")).toBeTruthy();
  });

  it("does not render dialog content when open is false", () => {
    const wrapper = mountWithPlugins(WelcomeDialog, {
      props: { open: false, shortcut: "Ctrl+Shift+V" },
      global: { stubs },
    });
    expect(wrapper.find("#welcome-title").exists()).toBe(false);
  });

  it("includes the shortcut text in the steps", async () => {
    const wrapper = mountWithPlugins(WelcomeDialog, {
      props: { open: true, shortcut: "Alt+V" },
      global: { stubs },
    });
    await nextTick();
    const steps = wrapper.find("ol").text();
    expect(steps).toContain("Alt+V");
  });
});
