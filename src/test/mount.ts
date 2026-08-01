import { mount, type MountingOptions } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { createPinia, setActivePinia } from "pinia";
import type { Component } from "vue";
import zhCN from "../locales/zh-CN";
import enUS from "../locales/en-US";

/**
 * Mount a component with the global plugins (i18n + Pinia) that the app
 * provides in production.  Keeps component tests free of repetitive boilerplate.
 */
export function mountWithPlugins<T extends Component>(
  component: T,
  options: MountingOptions<T> = {},
) {
  const pinia = createPinia();
  setActivePinia(pinia);

  const i18n = createI18n({
    legacy: false,
    locale: "zh-CN",
    fallbackLocale: "en-US",
    messages: {
      "zh-CN": zhCN,
      "en-US": enUS,
    },
  });

  return mount(component, {
    ...options,
    global: {
      plugins: [i18n, pinia, ...(options.global?.plugins ?? [])],
      ...(options.global ?? {}),
    },
  });
}
