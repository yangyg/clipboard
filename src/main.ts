import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { i18n } from "./locales";
import { installTooltips } from "./utils/tooltips";
import "./styles/main.css";
import "./styles/settings.css";

// Disable WebView native context menu (Inspect / Reload, etc.)
document.addEventListener("contextmenu", (e) => e.preventDefault());
installTooltips();

const app = createApp(App);
app.use(createPinia());
app.use(i18n);
app.mount("#app");
