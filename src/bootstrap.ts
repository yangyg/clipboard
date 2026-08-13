/**
 * Shared app bootstrap for both window entries (main + tray menu). The two
 * entries used to duplicate the createApp + contextmenu guard + tooltips +
 * i18n wiring; `mountApp` keeps that in one place. Only difference is whether
 * Pinia is installed.
 */
import { createApp, type Component } from "vue";
import { createPinia } from "pinia";
import { i18n } from "./locales";
import { installTooltips } from "./utils/tooltips";
import "./styles/main.css";

export function mountApp(root: Component, options: { pinia?: boolean } = {}) {
  // Disable WebView native context menu (Inspect / Reload, etc.)
  document.addEventListener("contextmenu", (e) => e.preventDefault());
  installTooltips();

  const app = createApp(root);
  if (options.pinia) app.use(createPinia());
  app.use(i18n);
  app.mount("#app");
  return app;
}
